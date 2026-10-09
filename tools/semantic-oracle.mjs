// Semantic oracle: TypeScript 7.0.2's checker classifies every identifier token
// in a project's search files, normalized to the Rust reference tool's model.
//
//   TYPE_SCRIPT_SEMANTIC_API=/path/to/typescript-7.0.2/package \
//   TYPE_SCRIPT_NATIVE=/path/to/typescript-<platform>-7.0.2/package/lib/tsc \
//     node tools/semantic-oracle.mjs [--pretty] path/to/tsconfig.json
//
// The API package (its unstable sync API) and the native binary that serves it
// must both be exactly 7.0.2. Nothing is downloaded; setup failures throw.
//
// Positions. The sync API reports node pos/end and accepts positions as UTF-16
// code unit offsets, not UTF-8 bytes (verified with astral, non-ASCII, and
// escaped identifiers). Locations are one-based lines and one-based UTF-16
// columns with exclusive ends; \n, \r\n, \r, U+2028 and U+2029 break lines.
// Files are `/`-separated paths relative to the configuration's directory.
//
// Tokens. Every Identifier and PrivateIdentifier node reachable from each search
// file (program files that are neither default-library nor external-library
// files) is classified:
//   symbol  - a declared target, after following aliases (getAliasedSymbol);
//             shorthand property names use getShorthandAssignmentValueSymbol.
//             Standard library declarations count like any others.
//   member  - property, method, accessor, enum member, signature, or
//             object-literal member symbols, plus the built-in `arguments`,
//             `undefined`, and `globalThis`, which have no declarations.
//   none    - no symbol, the unknown symbol, or an unresolved placeholder.
// Library files. The native compiler reads its standard libraries from its own
// directory; the tool reads byte-identical copies vendored in this checkout. A
// default-library file in the compiler's directory is located at the vendored
// copy of the same name (`vendoredLibrary`), the only path normalization; a
// replacement library from a package keeps its own path.
// `syntax` marks positions that never enter lexical lookup: labels, JSX
// intrinsic tags and attribute names, member, `#private`, and binding-property
// names, meta properties, import attribute keys, `declare global`, and `as const`.
// Their queries expect UnsupportedTarget whatever TypeScript resolves them to
// (a note records when it resolves one to a declaration). Members and
// unresolved `a.b` property names also expect UnsupportedTarget, except a
// missing export of a pure namespace, module, or globalThis qualifier, which
// is unresolved. A value use through a type-only import or export (outside
// `typeof`) is unresolved: the Rust tool keeps the type-only restriction where
// TypeScript reports an error but resolves. A name that
// `export *` declarations supply with different targets is unresolved: the
// tool treats it as ambiguous, as ECMAScript does, where TypeScript reports
// TS2308 and keeps the first. Other none tokens expect UnresolvedTarget;
// symbols expect their groups.
// `roles` are syntactic: declaration (a lexical declaration's name), import,
// export (specifiers, `export =`/`export default` identifiers, and names of
// declarations with an `export` modifier), or use. `candidate` marks the
// tokens a name query selects (see references.md, Selection): declaration
// names, local import names, and exported alias names.
//
// Normalization. TypeScript merges every same-named declaration of one symbol
// table into one symbol. The Rust tool keeps one symbol per meaning unless the
// declarations validly merge, so each target's declarations are split into
// clusters: connected components of "merges with": function+function
// (overloads), interface+interface, class+interface, class+function (an
// ambient class; other pairs are binding errors), namespace+namespace,
// namespace+class, namespace+function, namespace+enum, enum+enum,
// var/parameter+var (function-scoped redeclaration), and the type parameters
// that merged class and interface declarations share. Module augmentations and
// ambient modules count as namespaces, so they merge with their targets by the
// same rules. Anything else sharing a name (interface + declare var, type
// alias + const, non-instantiated namespace + const) stays separate.
// Cluster meanings: class = value+type; interface, type alias, and type
// parameter = type; enum = value+type+namespace; function, variable, and
// parameter = value; namespace = namespace (+value when TypeScript marks the
// symbol instantiated); modules and anonymous default/`export =` symbols =
// value(+namespace for modules).
// Token meanings: a declaration name belongs to its own declaration's cluster,
// even where TypeScript returns another symbol (a parameter property's name
// resolves to the property). Import/export specifier names, import clause,
// namespace import/export and import-equals names, and `export =`/`export
// default` identifiers take every meaning. Type references, interface
// `extends`, and `implements` are types; their left qualifiers, and the left
// qualifiers of import-equals entity names, are namespaces. `typeof a.b` uses
// value lookup, as do all other expressions and JSX tag names. A token belongs
// to each target cluster whose meanings intersect its own.
// Declarations are located at their name token. Module symbols are named by
// the file's path and located at {1:1-1:1}; ambient modules use the unquoted
// name and the string literal token; anonymous `export default` declarations
// use `default` and the `default` keyword; `export = <expression>` uses
// `export=` and the `=` token. Clusters with identical declaration sets are
// one group, even when TypeScript returns distinct (transient) symbols.
// Normalization never drops an occurrence: tokens with no matching cluster are
// reported as anomalies. Each token whose expected outcome departs from what
// TypeScript resolves, under a documented difference, carries a stable
// `normalization` code (see NORMALIZATIONS) so reports can count them.
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { VENDORED } from './vendor-typescript-libs.mjs';

export const SEMANTIC_VERSION = '7.0.2';

export const VALUE = 1, TYPE = 2, NAMESPACE = 4, ALL = 7;
const MEANING_NAMES = [[VALUE, 'value'], [TYPE, 'type'], [NAMESPACE, 'namespace']];
export const meaningNames = bits => MEANING_NAMES.filter(([bit]) => bits & bit).map(([, name]) => name);
const meaningBits = names => names.reduce((bits, name) => bits | MEANING_NAMES.find(([, n]) => n === name)[0], 0);

const CATEGORY_MEANING = {
  function: VALUE, class: VALUE | TYPE, interface: TYPE, type: TYPE, typeParameter: TYPE,
  enum: VALUE | TYPE | NAMESPACE, namespace: NAMESPACE, module: VALUE | NAMESPACE, export: VALUE,
  parameter: VALUE, var: VALUE, let: VALUE, const: VALUE, using: VALUE, 'await using': VALUE, catch: VALUE,
};
const MERGES = new Set(['function+function', 'interface+interface', 'class+interface', 'class+function', 'namespace+namespace',
  'class+namespace', 'function+namespace', 'enum+namespace', 'enum+enum', 'var+var', 'parameter+var',
  'typeParameter+typeParameter']);
const mergeKind = category => category === 'module' ? 'namespace' : category;
export const merges = (a, b) => MERGES.has([mergeKind(a), mergeKind(b)].sort().join('+'));
const MEMBER_FLAGS = ['Property', 'EnumMember', 'Method', 'GetAccessor', 'SetAccessor', 'Signature', 'Prototype', 'Constructor'];
// Names that never enter lexical lookup, whatever TypeScript resolves them to.
const NON_REFERENCE_SYNTAX = new Set(['label', 'jsxIntrinsic', 'jsxAttribute', 'jsxNamespacedName', 'memberName',
  'privateName', 'propertyName', 'metaProperty', 'importAttribute', 'globalAugmentation', 'constAssertion',
  'thisParameter', 'thisTypeQuery', 'tupleLabel']);

export function semanticCompilerVersion(binary) {
  const result = spawnSync(binary, ['--version'], { encoding: 'utf8', timeout: 10_000 });
  if (result.error || result.signal || result.status !== 0 || result.stdout.trim() !== `Version ${SEMANTIC_VERSION}`) {
    throw new Error(`TYPE_SCRIPT_NATIVE must point to the TypeScript ${SEMANTIC_VERSION} native compiler: ${result.error?.message || result.stderr?.trim() || result.stdout?.trim() || `exit ${result.status}, signal ${result.signal}`}`);
  }
  return result.stdout.trim();
}

// The package directory defaults to an installed `typescript` package.
export async function loadSemanticApi({ api = process.env.TYPE_SCRIPT_SEMANTIC_API, native = process.env.TYPE_SCRIPT_NATIVE } = {}) {
  let directory = api;
  if (!directory) {
    try { directory = path.dirname(createRequire(import.meta.url).resolve('typescript/package.json')); }
    catch { throw new Error('TYPE_SCRIPT_SEMANTIC_API must name the TypeScript 7.0.2 package directory'); }
  }
  let version;
  try { ({ version } = JSON.parse(fs.readFileSync(path.join(directory, 'package.json'), 'utf8'))); }
  catch (error) { throw new Error(`TYPE_SCRIPT_SEMANTIC_API has no readable package.json: ${error.message}`); }
  if (version !== SEMANTIC_VERSION) throw new Error(`Expected semantic API ${SEMANTIC_VERSION}, received ${version}`);
  if (!native) throw new Error('TYPE_SCRIPT_NATIVE must name the TypeScript 7.0.2 native compiler');
  const binary = path.resolve(native);
  const nativeVersion = semanticCompilerVersion(binary);
  const sync = await import(pathToFileURL(path.join(directory, 'dist/api/sync/api.js')).href);
  const ast = await import(pathToFileURL(path.join(directory, 'dist/ast/index.js')).href);
  return { version, nativeVersion, binary, API: sync.API, SymbolFlags: sync.SymbolFlags, ModifierFlags: sync.ModifierFlags,
    SyntaxKind: ast.SyntaxKind, NodeFlags: ast.NodeFlags, skipTrivia: ast.skipTrivia };
}

// ---------------------------------------------------------------------------
// Locations

export function lineStarts(text) {
  const starts = [0];
  for (let index = 0; index < text.length; index++) {
    const code = text.charCodeAt(index);
    if (code === 13) {
      if (text.charCodeAt(index + 1) === 10) index++;
      starts.push(index + 1);
    } else if (code === 10 || code === 0x2028 || code === 0x2029) starts.push(index + 1);
  }
  return starts;
}

export function position(starts, offset) {
  let low = 0, high = starts.length - 1;
  while (low < high) {
    const middle = (low + high + 1) >> 1;
    if (starts[middle] <= offset) low = middle; else high = middle - 1;
  }
  return { line: low + 1, column: offset - starts[low] + 1 };
}

export const locationKey = location => `${location.file}:${location.start.line}:${location.start.column}`;
export const describeLocation = location =>
  `${location.file}:${location.start.line}:${location.start.column}-${location.end.line}:${location.end.column}`;
export function compareLocations(a, b) {
  return a.file < b.file ? -1 : a.file > b.file ? 1
    : a.start.line - b.start.line || a.start.column - b.start.column || a.end.line - b.end.line || a.end.column - b.end.column;
}

// The vendored copy of a standard library the compiler read from its own
// directory `compilerLibraries`, by file name; undefined for any other file.
export function vendoredLibrary(fileName, compilerLibraries, vendored = VENDORED) {
  let real;
  try { real = fs.realpathSync(fileName); } catch { return undefined; }
  if (path.dirname(real) !== compilerLibraries) return undefined;
  const copy = path.join(vendored, path.basename(real));
  if (!fs.existsSync(copy)) throw new Error(`${path.basename(real)} has no vendored copy in ${vendored}`);
  return copy;
}

// Canonical paths, as the tool names files, unless symlinks are preserved:
// then a link and its target are distinct program files.
function displayPath(root, fileName, preserveSymlinks = false) {
  let real = fileName;
  if (!preserveSymlinks) {
    try { real = fs.realpathSync(fileName); } catch { /* keep the compiler's name */ }
  }
  return path.relative(root, real).split(path.sep).join('/');
}

// ---------------------------------------------------------------------------
// Collection: one TypeScript server, any number of projects.

export class SemanticSession {
  constructor(semantic) {
    this.semantic = semantic;
    this.api = new semantic.API({ tsserverPath: semantic.binary, cwd: process.cwd() });
  }

  close() { this.api.close(); }

  // Returns the raw oracle record consumed by `normalizeOracle`. Semantic
  // diagnostics are informational and can be skipped for large projects.
  collect(config, { diagnostics = true } = {}) {
    const configPath = fs.realpathSync(path.resolve(config));
    const snapshot = this.api.updateSnapshot({ openProject: configPath });
    try {
      const project = snapshot.getProject(configPath);
      if (!project) throw new Error(`TypeScript did not open ${configPath}`);
      return new Collector(this.semantic, project, configPath).run(diagnostics);
    } finally {
      snapshot.dispose();
    }
  }
}

class Collector {
  constructor(semantic, project, configPath) {
    this.k = semantic.SyntaxKind;
    this.flags = semantic.SymbolFlags;
    this.semantic = semantic;
    this.project = project;
    this.checker = project.checker;
    this.program = project.program;
    this.configPath = configPath;
    this.root = path.dirname(configPath);
    this.preserveSymlinks = Boolean(this.program.getCompilerOptions().preserveSymlinks);
    this.compilerLibraries = fs.realpathSync(path.dirname(semantic.binary));
    this.files = new Map();
    this.symbols = [];
    this.symbolIndex = new Map();
    this.targets = new Map();
    this.lines = new Map();
  }

  run(withDiagnostics) {
    const { program } = this;
    const fatal = [...program.getConfigFileParsingDiagnostics(), ...program.getProgramDiagnostics(), ...program.getSyntacticDiagnostics()]
      .filter(diagnostic => diagnostic.category === 1);
    if (fatal.length) {
      throw new Error(`TypeScript rejects ${this.configPath}: ${fatal.slice(0, 5).map(d => `${d.fileName ? displayPath(this.root, d.fileName, this.preserveSymlinks) + ': ' : ''}TS${d.code} ${d.text}`).join('; ')}`);
    }
    for (const fileName of program.getSourceFileNames()) this.fileInfo(fileName);
    const tokens = [];
    for (const [fileName, info] of this.files) if (info.search) tokens.push(...this.fileTokens(fileName));
    const diagnostics = !withDiagnostics ? [] : program.getSemanticDiagnostics().map(d => ({ file: d.fileName ? displayPath(this.root, d.fileName, this.preserveSymlinks) : null,
      ...(d.fileName ? this.location(program.getSourceFile(d.fileName), d.pos, d.end) : {}), code: d.code, message: d.text }));
    return { compiler: this.semantic.version, project: this.configPath, files: [...this.files.values()], symbols: this.symbols, tokens, diagnostics };
  }

  fileInfo(fileName) {
    let info = this.files.get(fileName);
    if (!info) {
      const metadata = this.program.getSourceFileMetadata(fileName);
      if (!metadata) throw new Error(`${fileName} is not a program file`);
      const vendored = metadata.isDefaultLibrary ? vendoredLibrary(fileName, this.compilerLibraries) : undefined;
      this.files.set(fileName, info = { file: displayPath(this.root, vendored ?? fileName, this.preserveSymlinks), search: !metadata.isDefaultLibrary && !metadata.isFromExternalLibrary,
        defaultLibrary: metadata.isDefaultLibrary, external: metadata.isFromExternalLibrary });
    }
    return info;
  }

  location(sourceFile, start, end) {
    let starts = this.lines.get(sourceFile.fileName);
    if (!starts) this.lines.set(sourceFile.fileName, starts = lineStarts(sourceFile.text));
    return { file: this.fileInfo(sourceFile.fileName).file, start: position(starts, start), end: position(starts, end) };
  }

  fileTokens(fileName) {
    const { k } = this;
    const sourceFile = this.program.getSourceFile(fileName);
    const identifiers = [];
    const ignored = this.semantic.NodeFlags.JSDoc | this.semantic.NodeFlags.Reparsed;
    const pending = [sourceFile];
    while (pending.length) {
      const node = pending.pop();
      if (node.flags & ignored) continue;
      if (node.kind === k.Identifier || node.kind === k.PrivateIdentifier) identifiers.push(node);
      const children = [];
      node.forEachChild(child => { children.push(child); });
      for (let index = children.length; index--;) pending.push(children[index]);
    }
    const symbols = [];
    for (let index = 0; index < identifiers.length; index += 2000)
      symbols.push(...this.checker.getSymbolAtLocation(identifiers.slice(index, index + 2000)));
    return identifiers.map((identifier, index) => this.token(sourceFile, identifier, symbols[index]));
  }

  token(sourceFile, identifier, symbol) {
    const { k } = this;
    const parent = identifier.parent;
    if (parent.kind === k.ShorthandPropertyAssignment && parent.name === identifier)
      symbol = this.checker.getShorthandAssignmentValueSymbol(parent);
    const target = symbol && this.target(symbol);
    const context = this.context(identifier);
    // An import or export clause name denotes only what it aliases.
    const clause = context.roles.includes('import') || context.roles.includes('export');
    const token = { location: this.location(sourceFile, identifier.getStart(sourceFile), identifier.end), name: identifier.text,
      symbol: !target ? null : this.recordWithLocals(symbol, target, !clause),
      meaning: meaningNames(context.meaning ?? 0), roles: context.roles };
    if (context.syntax) token.syntax = context.syntax;
    if (target?.escapedName === '__global') token.syntax = 'globalAugmentation';
    if (context.syntax === 'property') {
      // The receiver's last name, whose expected outcome the property shares.
      const qualifier = parent.kind === k.QualifiedName ? parent.left : parent.expression;
      const receiver = qualifier.kind === k.PropertyAccessExpression ? qualifier.name
        : qualifier.kind === k.QualifiedName ? qualifier.right : qualifier;
      if (receiver.kind === k.Identifier) token.receiver = this.location(sourceFile, receiver.getStart(sourceFile), receiver.end);
    }
    if (context.syntax === 'property' && (!target || this.checker.isUnknownSymbol(target))) {
      const qualifier = this.checker.getSymbolAtLocation(parent.kind === k.QualifiedName ? parent.left : parent.expression);
      if (qualifier && this.pureNamespace(this.target(qualifier))) token.namespaceQualifier = true;
    }
    if (symbol && symbol.flags & this.flags.Alias && meaningBits(token.meaning) === VALUE
      && !this.inTypeQuery(identifier) && this.typeOnlyChain(symbol)) token.typeOnlyValue = true;
    if (target && !this.checker.isUnknownSymbol(target) && this.ambiguousStar(identifier, symbol)) token.ambiguousStar = true;
    const declares = this.declaredBy(identifier);
    if (declares) token.declares = this.declaration(declares);
    if (this.candidate(identifier, context)) token.candidate = true;
    return token;
  }

  // Whether a name query selects this token: a declaration name, a local
  // import name, or an exported alias name, but not the imported or
  // re-exported name before `as`.
  candidate(identifier, context) {
    const { k } = this;
    const parent = identifier.parent;
    if (context.syntax) return false;
    if (context.roles.includes('declaration')) return true;
    switch (parent.kind) {
      case k.ImportSpecifier: case k.ExportSpecifier: case k.ImportEqualsDeclaration:
        return parent.name === identifier;
      case k.ImportClause: case k.NamespaceImport: case k.NamespaceExport: case k.NamespaceExportDeclaration:
        return true;
      default:
        return false;
    }
  }

  target(symbol) {
    if (!(symbol.flags & this.flags.Alias)) return symbol;
    let target = this.targets.get(symbol.id);
    if (!target) this.targets.set(symbol.id, target = this.checker.getAliasedSymbol(symbol));
    return target;
  }

  record(symbol) {
    let index = this.symbolIndex.get(symbol.id);
    if (index !== undefined) return index;
    const { checker } = this;
    const special = checker.isUnknownSymbol(symbol) ? 'unknown' : checker.isUndefinedSymbol(symbol) ? 'undefined'
      : checker.isArgumentsSymbol(symbol) ? 'arguments' : undefined;
    const flags = Object.entries(this.flags).filter(([, bit]) => typeof bit === 'number' && bit && !(bit & (bit - 1)) && (symbol.flags & bit)).map(([name]) => name);
    const declarations = (symbol.declarations ?? []).map(handle => {
      const node = handle.resolve(this.project);
      if (!node) throw new Error(`Cannot resolve a declaration of ${symbol.name} in ${handle.path}`);
      return this.declaration(node);
    });
    const record = { name: symbol.name, flags, declarations };
    if (special) record.special = special;
    index = this.symbols.push(record) - 1;
    this.symbolIndex.set(symbol.id, index);
    return index;
  }

  // Each immediate alias can also declare independent local meanings.
  // getAliasedSymbol may stop at such a mixed symbol, so capture the whole
  // chain. Import/export clause names skip their own local declarations.
  recordWithLocals(symbol, target, includeOwn = true) {
    if (!(symbol.flags & this.flags.Alias)) return this.record(target);
    this.combined ??= new Map();
    const key = `${symbol.id}:${includeOwn}`;
    if (this.combined.has(key)) return this.combined.get(key);
    const records = [], seen = new Set();
    let current = symbol;
    while (current && !seen.has(current.id)) {
      seen.add(current.id);
      if (!(current.flags & this.flags.Alias)) {
        records.push(this.symbols[this.record(current)]);
        break;
      }
      const local = current.flags & (this.flags.Value | this.flags.Type | this.flags.Namespace);
      if (local && (current !== symbol || includeOwn)) {
        const record = this.symbols[this.record(current)];
        const declarations = record.declarations.filter(d => d.category !== 'alias');
        if (declarations.length) records.push({ ...record, declarations });
      }
      const next = this.checker.getImmediateAliasedSymbol(current);
      if (!next || seen.has(next.id)) {
        records.push(this.symbols[this.record(this.target(current))]);
        break;
      }
      current = next;
    }
    const declarations = [...new Map(records.flatMap(r => r.declarations)
      .map(d => [declarationKey(d), d])).values()];
    const combined = { name: target.name,
      flags: [...new Set(records.flatMap(r => r.flags))], declarations };
    if (!declarations.length && records.at(-1)?.special) combined.special = records.at(-1).special;
    const index = this.symbols.push(combined) - 1;
    this.combined.set(key, index);
    return index;
  }

  // Whether the token reaches its target through `export *` declarations that
  // supply different targets for its name, at an import or re-export along
  // its alias chain or as a member of a module receiver. ECMAScript makes such
  // a name ambiguous, as the tool does; TypeScript reports TS2308 at the
  // conflicting `export *` and keeps the first target.
  ambiguousStar(identifier, symbol) {
    const { k } = this;
    const parent = identifier.parent;
    if ((parent.kind === k.PropertyAccessExpression && parent.name === identifier)
      || (parent.kind === k.QualifiedName && parent.right === identifier)) {
      const qualifier = this.checker.getSymbolAtLocation(parent.kind === k.QualifiedName ? parent.left : parent.expression);
      const module = qualifier && this.target(qualifier);
      return !!module && !this.checker.isUnknownSymbol(module)
        && this.symbols[this.record(module)].declarations.some(d => d.category === 'module')
        && this.starConflict(module, identifier.text);
    }
    const conflicts = node => {
      if (node?.kind !== k.ImportSpecifier && node?.kind !== k.ExportSpecifier) return false;
      const declaration = node.kind === k.ImportSpecifier ? node.parent.parent.parent : node.parent.parent;
      const module = declaration.moduleSpecifier && this.checker.getSymbolAtLocation(declaration.moduleSpecifier);
      return !!module && this.starConflict(module, (node.propertyName ?? node.name).text);
    };
    // The imported or re-exported name before `as` denotes the module's export.
    if (parent.propertyName === identifier && conflicts(parent)) return true;
    const seen = new Set();
    for (let current = symbol; current && current.flags & this.flags.Alias && !seen.has(current.id);
      current = this.checker.getImmediateAliasedSymbol(current)) {
      seen.add(current.id);
      if ((current.declarations ?? []).some(handle => conflicts(handle.resolve(this.project)))) return true;
    }
    return false;
  }

  starConflict(module, name) {
    this.conflicts ??= new Map();
    const key = `${module.id}:${name}`;
    if (!this.conflicts.has(key)) this.conflicts.set(key, this.findStarConflict(module, name, new Set()));
    return this.conflicts.get(key);
  }

  // A name that a module does not export itself is looked up through its
  // `export *` declarations (TypeScript's `__export` entry); it conflicts when
  // they resolve it to different targets or a nested lookup conflicts.
  findStarConflict(module, name, visiting) {
    if (visiting.has(module.id)) return false;
    visiting.add(module.id);
    const exports = module.getExports();
    if (exports.has(name.startsWith('__') ? `_${name}` : name) || exports.has('export=')) return false;
    const targets = new Set();
    for (const handle of exports.get('__export')?.declarations ?? []) {
      const node = handle.resolve(this.project);
      const next = node?.moduleSpecifier && this.checker.getSymbolAtLocation(node.moduleSpecifier);
      if (!next) continue;
      if (this.findStarConflict(next, name, visiting)) return true;
      const member = this.checker.getMemberInModuleExports(next, name);
      if (member) targets.add(this.target(member).id);
    }
    return targets.size > 1;
  }

  // A namespace or module that is not also a class, function, variable, or
  // enum, whose missing members are unresolved rather than properties.
  pureNamespace(symbol) {
    const { flags } = this;
    if (symbol.name === 'globalThis' && !(symbol.declarations ?? []).length) return true;
    return !!(symbol.flags & (flags.ValueModule | flags.NamespaceModule))
      && !(symbol.flags & (flags.Class | flags.Function | flags.Variable | flags.Enum));
  }

  inTypeQuery(identifier) {
    const { k } = this;
    let node = identifier.parent;
    while (node && (node.kind === k.QualifiedName || node.kind === k.PropertyAccessExpression)) node = node.parent;
    return node?.kind === k.TypeQuery;
  }

  // Whether the value target is reached only through a type-only edge.
  // A same-named local value ends the lookup before that symbol's type alias.
  typeOnlyChain(symbol) {
    const { k, flags } = this;
    this.typeOnly ??= new Map();
    if (this.typeOnly.has(symbol.id)) return this.typeOnly.get(symbol.id);
    let result = false;
    const seen = new Set();
    for (let current = symbol; current && current.flags & flags.Alias && !seen.has(current.id);
      current = this.checker.getImmediateAliasedSymbol(current)) {
      seen.add(current.id);
      if (current.flags & flags.Value) {
        const own = this.symbols[this.record(current)];
        if (clusterDeclarations(own).some(cluster => cluster.meaning & VALUE)) break;
      }
      for (const handle of current.declarations ?? []) {
        const node = handle.resolve(this.project);
        if (!node) continue;
        // Only import and export clauses carry `isTypeOnly`; the API reports
        // other kinds' unrelated flags under that name.
        const own = [k.ImportClause, k.ImportSpecifier, k.ImportEqualsDeclaration, k.ExportSpecifier].includes(node.kind);
        const typeOnly = (own && node.isTypeOnly)
          || (node.kind === k.ImportSpecifier && node.parent.parent.isTypeOnly)
          || (node.kind === k.NamespaceImport && node.parent.isTypeOnly)
          || (node.kind === k.ExportSpecifier && node.parent.parent.isTypeOnly)
          || (node.kind === k.NamespaceExport && node.parent.isTypeOnly);
        if (typeOnly) result = true;
      }
      if (result) break;
    }
    this.typeOnly.set(symbol.id, result);
    return result;
  }

  // The declaration node whose name is `identifier`, for lexical declarations.
  declaredBy(identifier) {
    const { k } = this;
    const parent = identifier.parent;
    if (parent.name !== identifier) return undefined;
    switch (parent.kind) {
      case k.VariableDeclaration: case k.BindingElement: case k.Parameter: case k.FunctionDeclaration: case k.FunctionExpression:
      case k.ClassDeclaration: case k.ClassExpression: case k.InterfaceDeclaration: case k.TypeAliasDeclaration:
      case k.EnumDeclaration: case k.TypeParameter:
        return parent;
      case k.ModuleDeclaration:
        return identifier.text === 'global' && parent.keyword === k.ModuleKeyword && (parent.flags & this.semantic.NodeFlags.Ambient) ? undefined : parent;
      default:
        return undefined;
    }
  }

  declaration(node) {
    const { k } = this;
    const sourceFile = node.getSourceFile();
    const info = this.fileInfo(sourceFile.fileName);
    const category = this.category(node);
    let name, location;
    if (node.kind === k.SourceFile) {
      name = info.file;
      location = { file: info.file, start: { line: 1, column: 1 }, end: { line: 1, column: 1 } };
    } else if (node.kind === k.ModuleDeclaration && node.name.kind === k.StringLiteral) {
      name = node.name.text;
      location = this.location(sourceFile, node.name.getStart(sourceFile), node.name.end);
    } else if (node.kind === k.ExportAssignment) {
      const text = sourceFile.text;
      const keyword = this.semantic.skipTrivia(text, node.getStart(sourceFile) + 'export'.length);
      const spelling = node.isExportEquals ? '=' : 'default';
      if (!text.startsWith(spelling, keyword)) throw new Error(`Cannot find \`${spelling}\` in an export assignment in ${info.file}`);
      name = node.isExportEquals ? 'export=' : 'default';
      location = this.location(sourceFile, keyword, keyword + spelling.length);
    } else if (node.name?.kind === k.Identifier) {
      name = node.name.text;
      location = this.location(sourceFile, node.name.getStart(sourceFile), node.name.end);
    } else {
      const keyword = node.modifiers?.find(modifier => modifier.kind === k.DefaultKeyword);
      name = keyword ? 'default' : '';
      const at = keyword ?? node;
      location = this.location(sourceFile, at.getStart(sourceFile), keyword ? keyword.end : at.getStart(sourceFile));
    }
    const record = { name, location, category, kind: syntaxKindName(k, node.kind) };
    if (info.defaultLibrary) record.defaultLibrary = true;
    if (info.external) record.external = true;
    return record;
  }

  category(node) {
    const { k } = this;
    switch (node.kind) {
      case k.FunctionDeclaration: case k.FunctionExpression: return 'function';
      case k.ClassDeclaration: case k.ClassExpression: return 'class';
      case k.InterfaceDeclaration: return 'interface';
      case k.TypeAliasDeclaration: return 'type';
      case k.EnumDeclaration: return 'enum';
      case k.TypeParameter: return 'typeParameter';
      case k.SourceFile: return 'module';
      case k.ModuleDeclaration: return node.name.kind === k.StringLiteral ? 'module' : 'namespace';
      case k.ExportAssignment: return 'export';
      case k.Parameter: return 'parameter';
      case k.VariableDeclaration: case k.BindingElement: {
        let root = node;
        while (root.kind !== k.VariableDeclaration && root.kind !== k.Parameter) root = root.parent;
        if (root.kind === k.Parameter) return 'parameter';
        if (root.parent.kind === k.CatchClause) return 'catch';
        const flags = root.parent.flags & 7;
        return ['var', 'let', 'const', 'const', 'using', 'using', 'await using', 'await using'][flags];
      }
      case k.ImportSpecifier: case k.ImportClause: case k.NamespaceImport: case k.ImportEqualsDeclaration:
      case k.ExportSpecifier: case k.NamespaceExport: case k.NamespaceExportDeclaration:
        return 'alias';
      default: return 'member';
    }
  }

  // Meaning, non-reference syntax, and syntactic roles of one identifier.
  context(identifier) {
    const { k } = this;
    const parent = identifier.parent;
    // `#name` declares or accesses a class member, and `#name in object` tests one.
    if (identifier.kind === k.PrivateIdentifier) return { syntax: 'privateName', roles: [] };
    const exported = this.exported(parent);
    switch (parent.kind) {
      case k.LabeledStatement: case k.BreakStatement: case k.ContinueStatement: return { syntax: 'label', roles: [] };
      case k.ImportSpecifier: case k.ImportClause: case k.NamespaceImport: return { meaning: ALL, roles: ['import'] };
      case k.ImportEqualsDeclaration:
        if (parent.name === identifier) return { meaning: ALL, roles: exported ? ['export', 'import'] : ['import'] };
        break;
      case k.ExportSpecifier: case k.NamespaceExport: case k.NamespaceExportDeclaration: return { meaning: ALL, roles: ['export'] };
      case k.ExportAssignment: return { meaning: ALL, roles: ['export'] };
      case k.JsxAttribute: return { syntax: 'jsxAttribute', roles: [] };
      case k.JsxNamespacedName: return { syntax: 'jsxNamespacedName', roles: [] };
      case k.MetaProperty: return { syntax: 'metaProperty', roles: [] };
      case k.NamedTupleMember: if (parent.name === identifier) return { syntax: 'tupleLabel', roles: [] }; break;
      case k.Parameter: if (parent.name === identifier && identifier.text === 'this') return { syntax: 'thisParameter', roles: [] }; break;
      // `typeof this` and `typeof this.x` name the enclosing class's instance.
      case k.TypeQuery: if (identifier.text === 'this') return { syntax: 'thisTypeQuery', roles: [] }; break;
      case k.QualifiedName:
        if (parent.left === identifier && identifier.text === 'this') {
          let up = parent;
          while (up.kind === k.QualifiedName) up = up.parent;
          if (up.kind === k.TypeQuery) return { syntax: 'thisTypeQuery', roles: [] };
        }
        break;
      case k.ImportAttribute: return { syntax: 'importAttribute', roles: [] };
      case k.JsxOpeningElement: case k.JsxSelfClosingElement: case k.JsxClosingElement:
        if (/^[a-z]|-/.test(identifier.text)) return { syntax: 'jsxIntrinsic', roles: [] };
        return { meaning: VALUE, roles: ['use'] };
      case k.BindingElement:
        if (parent.propertyName === identifier) return { syntax: 'propertyName', roles: [] };
        break;
      case k.PropertyAssignment: case k.PropertyDeclaration: case k.PropertySignature: case k.MethodDeclaration:
      case k.MethodSignature: case k.GetAccessor: case k.SetAccessor: case k.EnumMember:
        if (parent.name === identifier) return { syntax: 'memberName', roles: [] };
        break;
      case k.ShorthandPropertyAssignment:
        if (parent.name === identifier) return { meaning: VALUE, roles: ['use'] };
        break;
      case k.TypeReference:
        // `x as const` and `<const>x` spell the keyword as a type name.
        if (identifier.text === 'const' && (parent.parent.kind === k.AsExpression || parent.parent.kind === k.TypeAssertionExpression))
          return { syntax: 'constAssertion', roles: [] };
        break;
    }
    if (this.declaredBy(identifier)) return { meaning: ALL, roles: exported ? ['declaration', 'export'] : ['declaration'] };
    // An entity name or property access chain: find its outermost node.
    let top = identifier;
    for (;;) {
      const up = top.parent;
      if (up.kind === k.QualifiedName && (up.left === top || up.right === top)) top = up;
      else if (up.kind === k.PropertyAccessExpression && (up.expression === top || up.name === top)) top = up;
      else break;
    }
    const meaning = this.chainMeaning(top);
    const right = top === identifier || top.right === identifier || top.name === identifier;
    // `export default a.b` qualifies a value or a namespace.
    const left = top.parent.kind === k.ExportAssignment ? VALUE | NAMESPACE : NAMESPACE;
    const result = { meaning: right || meaning === VALUE ? meaning : left, roles: ['use'] };
    if (identifier.parent.kind === k.PropertyAccessExpression && identifier.parent.name === identifier) result.syntax = 'property';
    // `typeof a.b` accesses a property of a value, as an expression does.
    if (identifier.parent.kind === k.QualifiedName && identifier.parent.right === identifier && top.parent.kind === k.TypeQuery)
      result.syntax = 'property';
    return result;
  }

  chainMeaning(node) {
    const { k } = this;
    const parent = node.parent;
    switch (parent.kind) {
      case k.TypeReference: return TYPE;
      case k.ImportType: return parent.isTypeOf ? VALUE : TYPE;
      case k.ImportEqualsDeclaration: return ALL;
      case k.ExportAssignment: return ALL;
      case k.ExpressionWithTypeArguments: {
        const clause = parent.parent;
        if (clause.kind === k.HeritageClause && (clause.token === k.ImplementsKeyword || clause.parent.kind === k.InterfaceDeclaration)) return TYPE;
        return VALUE;
      }
      default: return VALUE;
    }
  }

  exported(node) {
    const { k } = this;
    let declaration = node;
    while (declaration.kind === k.BindingElement || declaration.kind === k.ObjectBindingPattern || declaration.kind === k.ArrayBindingPattern) declaration = declaration.parent;
    if (declaration.kind === k.VariableDeclaration && declaration.parent.kind === k.VariableDeclarationList) declaration = declaration.parent.parent;
    return !!declaration.modifiers?.some(modifier => modifier.kind === k.ExportKeyword);
  }
}

function syntaxKindName(k, kind) {
  // Several enum members alias one value (FirstNode, FirstToken, ...); prefer a concrete name.
  const names = Object.keys(k).filter(name => k[name] === kind && !/^(First|Last)/.test(name));
  return names[0] ?? String(kind);
}

// ---------------------------------------------------------------------------
// Normalization: a pure function of the raw record, so it is testable without
// a compiler.

export function classifySymbol(symbol) {
  if (!symbol) return 'none';
  if (symbol.special === 'undefined' || symbol.special === 'arguments') return 'member';
  if (symbol.special === 'unknown') return 'none';
  const declarations = symbol.declarations;
  if (declarations.some(declaration => declaration.category === 'export')) return 'symbol';
  if (symbol.flags.some(flag => MEMBER_FLAGS.includes(flag))) return 'member';
  if (!declarations.length) return symbol.name === 'globalThis' ? 'member' : 'none';
  if (declarations.every(declaration => declaration.category === 'member' || declaration.category === 'alias')) return 'member';
  return 'symbol';
}

export const declarationKey = declaration => `${locationKey(declaration.location)}:${declaration.category}`;

// Connected components of the merges-with relation, with their meanings.
export function clusterDeclarations(symbol) {
  const declarations = symbol.declarations.filter(declaration => CATEGORY_MEANING[declaration.category] !== undefined);
  const parents = declarations.map((_, index) => index);
  const find = index => parents[index] === index ? index : (parents[index] = find(parents[index]));
  for (let a = 0; a < declarations.length; a++)
    for (let b = a + 1; b < declarations.length; b++)
      if (merges(declarations[a].category, declarations[b].category)) parents[find(a)] = find(b);
  const components = new Map();
  declarations.forEach((declaration, index) => {
    const root = find(index);
    if (!components.has(root)) components.set(root, []);
    components.get(root).push(declaration);
  });
  const instantiated = symbol.flags.includes('ValueModule');
  return [...components.values()].map(members => {
    let meaning = 0;
    for (const declaration of members) {
      meaning |= CATEGORY_MEANING[declaration.category];
      if (declaration.category === 'namespace' && instantiated) meaning |= VALUE;
    }
    members.sort((a, b) => compareLocations(a.location, b.location));
    return { declarations: members, meaning, key: members.map(declarationKey).sort().join('|') };
  });
}

// Codes for tokens whose expected outcome departs from TypeScript's
// resolution under a difference documented in references.md ("Differences
// from TypeScript 7.0.2" and "Compatibility limits"):
//   nonReference:<syntax> - TypeScript resolves a non-reference name (a `this`
//                           parameter, tuple label, binding key, ...): unsupported.
//   typeOnlyValue         - a value use through a type-only alias: unresolved.
//   ambiguousStarExport   - a name that `export *` declarations supply with
//                           different targets, where TypeScript keeps the
//                           first (TS2308): unresolved.
//   builtin               - `undefined`, `arguments`, `globalThis`: unsupported.
//   missingNamespaceExport - a missing export of a namespace qualifier: unresolved.
//   unresolvedProperty    - any other unresolved `a.b` name: unsupported.
//   unresolvedReceiver    - a property of an unresolved receiver: unsupported.
//   receiverType          - resolved through a receiver's type: unsupported.
//   declarationOutsideTarget - a declaration name TypeScript resolves to
//                           another symbol (a parameter property): own group.
export function normalizationCode(token) {
  if (NON_REFERENCE_SYNTAX.has(token.syntax)) return token.classification === 'symbol' ? `nonReference:${token.syntax}` : undefined;
  if (token.typeOnlyValue && token.classification === 'symbol') return 'typeOnlyValue';
  if (token.ambiguousStar && token.classification === 'symbol') return 'ambiguousStarExport';
  if (token.builtin) return 'builtin';
  if (token.classification === 'none' && token.syntax === 'property') return token.namespaceQualifier ? 'missingNamespaceExport' : 'unresolvedProperty';
  if (token.outsideTarget) return 'declarationOutsideTarget';
  return undefined;
}

export function expectedOutcome(token) {
  if (NON_REFERENCE_SYNTAX.has(token.syntax)) return 'unsupported';
  if (token.typeOnlyValue || token.ambiguousStar) return 'unresolved';
  if (token.classification === 'symbol') return token.groups.length ? 'groups' : 'anomaly';
  if (token.classification === 'member') return 'unsupported';
  // An unresolved `a.b` name is a member access on an unknown receiver,
  // unless the receiver is a namespace whose export is missing.
  if (token.classification === 'none' && token.syntax === 'property') {
    return token.namespaceQualifier ? 'unresolved' : 'unsupported';
  }
  return 'unresolved';
}

export function normalizeOracle(raw) {
  const groups = [], groupIndex = new Map(), anomalies = [];
  const clusterCache = new Map();
  const group = cluster => {
    let index = groupIndex.get(cluster.key);
    if (index === undefined) {
      index = groups.push({ declarations: cluster.declarations.map(({ name, location }) => ({ name, location })), occurrences: [], meaning: cluster.meaning }) - 1;
      groupIndex.set(cluster.key, index);
    }
    return index;
  };
  const tokens = raw.tokens.map(token => {
    const symbol = token.symbol === null ? null : raw.symbols[token.symbol];
    const classification = classifySymbol(symbol);
    const result = { ...token, classification, groups: [] };
    if (classification === 'member' && (symbol.special === 'undefined' || symbol.special === 'arguments'
      || (symbol.name === 'globalThis' && !symbol.declarations.length))) result.builtin = true;
    if (NON_REFERENCE_SYNTAX.has(token.syntax)) {
      if (classification === 'symbol') result.note = `TypeScript resolves this non-reference name to \`${symbol.name}\``;
    } else if (token.declares && CATEGORY_MEANING[token.declares.category] !== undefined && classification !== 'none') {
      // A declaration name denotes its own declaration's cluster.
      const clusters = classification === 'symbol' ? clusterOf(symbol) : [];
      const own = clusters.find(cluster => cluster.declarations.some(d => declarationKey(d) === declarationKey(token.declares)));
      if (!own) {
        result.note = 'declaration outside the TypeScript target symbol';
        result.outsideTarget = true;
      }
      result.classification = 'symbol';
      result.groups = [group(own ?? clusterDeclarations({ declarations: [token.declares], flags: [] })[0])];
    } else if (classification === 'symbol') {
      const meaning = meaningBits(token.meaning);
      result.groups = clusterOf(symbol).filter(cluster => cluster.meaning & meaning).map(group);
      if (!result.groups.length) anomalies.push({ location: token.location, name: token.name,
        reason: `no declaration cluster of \`${symbol.name}\` has the token's meaning (${token.meaning.join('+') || 'none'})` });
    }
    result.outcome = expectedOutcome(result);
    const code = normalizationCode(result);
    if (code) result.normalization = code;
    return result;

    function clusterOf(target) {
      let clusters = clusterCache.get(token.symbol);
      if (!clusters) clusterCache.set(token.symbol, clusters = clusterDeclarations(target));
      return clusters;
    }
  });
  // A property of a receiver that cannot be resolved, or that is not a
  // namespace or module, is a property access: the tool does not infer a
  // receiver's type, even where TypeScript resolves the name through it.
  const byLocation = new Map(tokens.map(token => [locationKey(token.location), token]));
  for (const token of tokens) {
    const receiver = token.receiver && byLocation.get(locationKey(token.receiver));
    if (!receiver || token.outcome !== 'groups') continue;
    const target = receiver.symbol === null ? null : raw.symbols[receiver.symbol];
    const namespace = target?.flags.some(flag => flag === 'ValueModule' || flag === 'NamespaceModule');
    if (receiver.outcome === 'unresolved') {
      token.outcome = 'unsupported';
      token.note = 'property of an unresolved receiver';
      token.normalization = 'unresolvedReceiver';
    } else if (receiver.outcome === 'groups' && !namespace) {
      token.outcome = 'unsupported';
      token.note = "resolved through the receiver's type";
      token.normalization = 'receiverType';
    }
  }
  for (const token of tokens) {
    // A token expected to be unresolved or unsupported belongs to no group.
    if (token.outcome !== 'groups') token.groups = [];
    const roles = [...token.roles].sort();
    for (const index of token.groups) groups[index].occurrences.push({ name: token.name, location: token.location, roles });
  }
  // Groups are ordered by their earliest declaration; renumber the tokens.
  const order = groups.map((_, index) => index).sort((a, b) =>
    compareLocations(groups[a].declarations[0].location, groups[b].declarations[0].location));
  const renumber = new Map(order.map((old, index) => [old, index]));
  for (const token of tokens) token.groups = token.groups.map(index => renumber.get(index)).sort((a, b) => a - b);
  const sorted = order.map(index => {
    const { declarations, occurrences, meaning } = groups[index];
    occurrences.sort((a, b) => compareLocations(a.location, b.location));
    return { meaning: meaningNames(meaning), declarations, occurrences };
  });
  return { ...raw, tokens, groups: sorted, anomalies };
}

export async function semanticOracle(config, semantic) {
  const session = new SemanticSession(semantic ?? await loadSemanticApi());
  try { return normalizeOracle(session.collect(config)); } finally { session.close(); }
}

// The Rust result shape: groups without internal fields. Occurrence roles are
// sorted by name; compare them as sets.
export function resultGroups(oracle, indices = oracle.groups.map((_, index) => index)) {
  return indices.map(index => ({ declarations: oracle.groups[index].declarations, occurrences: oracle.groups[index].occurrences }));
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  try {
    const args = process.argv.slice(2);
    const configs = args.filter(arg => !arg.startsWith('--'));
    if (configs.length !== 1) throw new Error('Usage: node tools/semantic-oracle.mjs [--pretty] path/to/tsconfig.json');
    const oracle = await semanticOracle(configs[0]);
    process.stdout.write(JSON.stringify(oracle, null, args.includes('--pretty') ? 2 : undefined) + '\n');
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}

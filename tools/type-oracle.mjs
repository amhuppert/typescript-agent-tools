// TypeScript 7.0.2 type/member oracle. Each public request owns a fresh checker.
// Implementation is deliberately separate from the lexical reference oracle:
// member identity must not inherit its UnsupportedTarget normalizations.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { isDeepStrictEqual } from 'node:util';
import { loadSemanticApi, lineStarts, position, compareLocations, compareExactText, vendoredLibrary } from './semantic-oracle.mjs';
import { VENDORED } from './vendor-typescript-libs.mjs';
import { compilerText } from './navigation-oracle.mjs';
import { captureFileSystem } from './type-oracle-filesystem.mjs';
import { compiledOracle } from './type-oracle-compiler.mjs';



// GetSourceFiles/IsSourceFileDefaultLibrary/IsSourceFileFromExternalLibrary:
// validate the actual replay order and provenance. Only a pinned, text-identical
// default library can map its bundled virtual identity to the API disk identity.
function validatedReplayProgram(input, actual) {
  if (!Array.isArray(actual) || actual.length !== input.programFiles.length)
    throw new Error('Compiler hook replay program inventory is missing or incomplete');
  const seen = new Set();
  const programFiles = actual.map(row => {
    if (typeof row?.file !== 'string' || typeof row.defaultLibrary !== 'boolean'
        || typeof row.externalLibrary !== 'boolean' || !/^[0-9a-f]{64}$/.test(row.sourceTextSha256))
      throw new Error('Compiler hook replay program source metadata is invalid');
    let file = row.file;
    if (file.startsWith('bundled:///')) {
      const candidates = input.programFiles.filter(expected => expected.defaultLibrary
        && path.basename(expected.file) === path.basename(file));
      if (!row.defaultLibrary || candidates.length !== 1)
        throw new Error('Compiler hook bundled library has no unique pinned disk identity: '+file);
      file = candidates[0].file;
    }
    if (seen.has(file)) throw new Error('Compiler hook replay program has a duplicate source identity: '+file);
    seen.add(file);
    const captured = input.files[file];
    if (captured === undefined || createHash('sha256').update(compilerText(captured),'utf8').digest('hex') !== row.sourceTextSha256)
      throw new Error('Compiler hook replay program source text differs from captured bytes: '+file);
    return {file,defaultLibrary:row.defaultLibrary,externalLibrary:row.externalLibrary};
  });
  if (!isDeepStrictEqual(programFiles,input.programFiles))
    throw new Error('Compiler hook replay program source order or provenance differs from API capture');
  return programFiles;
}

async function freshRequest(config, action, captureInputs = false, currentDirectory) {
  const semantic = await loadSemanticApi();
  const configPath = fs.realpathSync(path.resolve(config));
  const capture = captureInputs ? captureFileSystem() : undefined;
  const cwd = currentDirectory === undefined ? path.dirname(configPath) : fs.realpathSync(path.resolve(currentDirectory));
  const api = new semantic.API({ tsserverPath: semantic.binary, cwd, ...(capture ? { fs: capture.callbacks } : {}) });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProject: configPath });
    const project = snapshot.getProject(configPath);
    if (!project) throw new Error(`TypeScript did not open ${configPath}`);
    const fatal = [...project.program.getConfigFileParsingDiagnostics(), ...project.program.getProgramDiagnostics(),
      ...project.program.getSyntacticDiagnostics(), ...project.program.getBindDiagnostics()];
    if (fatal.length) throw new Error(`TypeScript rejected the project: ${fatal.map(d => `TS${d.code} ${d.text}`).join('; ')}`);
    const caseProbe = path.join(path.dirname(configPath), '__TypeOracle_CASE_Probe__');
    if (capture && typeof api.toPath !== 'function') throw new Error('Pinned API canonical filename operation is missing');
    return action({ semantic, project, configPath, root: path.dirname(configPath), currentDirectory: cwd, checker: project.checker,
      ...(capture ? { capturedInputs: capture, caseSensitive: api.toPath(caseProbe) === caseProbe } : {}) });
  } finally {
    try { snapshot?.dispose(); } finally { api.close(); }
  }
}

function offsetAt(source, at) {
  const starts = lineStarts(source.text);
  if (!Number.isSafeInteger(at.line) || at.line < 1 || at.line > starts.length || !Number.isSafeInteger(at.column) || at.column < 1)
    throw new Error('Oracle positions require one-based line and UTF-16 column');
  const offset = starts[at.line - 1] + at.column - 1;
  if (offset >= (starts[at.line] ?? source.text.length)) throw new Error('Oracle position is outside its line');
  return offset;
}

function location(context, source, node) {
  const starts = lineStarts(source.text);
  const metadata = context.project.program.getSourceFileMetadata(source.fileName);
  const library = metadata?.isDefaultLibrary && vendoredLibrary(source.fileName, fs.realpathSync(path.dirname(context.semantic.binary)));
  return { file: path.relative(context.root, library || source.fileName).split(path.sep).join('/'),
    start: position(starts, node.getStart(source)), end: position(starts, node.end) };
}

function sourceOf(node) {
  let source = node;
  while (source.parent) source = source.parent;
  return source;
}

function origins(context, symbol) {
  const result = (symbol?.declarations ?? []).map(handle => {
    const node = handle.resolve(context.project);
    if (!node) throw new Error(`Cannot resolve a declaration of ${symbol.name} in ${handle.path}`);
    return { kind: context.semantic.SyntaxKind[node.kind], location: location(context, sourceOf(node), node.name ?? node) };
  }).sort(compareOrigins);
  return result.filter((origin, index) => index === 0 || compareOrigins(result[index - 1], origin) !== 0);
}

function selectedSyntax(context, at, allowJson = false) {
  const source = context.project.program.getSourceFile(path.resolve(context.root, at.file));
  if (!source) throw new Error(`${at.file} is not a program file`);
  if (!allowJson && /\.json$/i.test(source.fileName)) throw new Error('UnsupportedTarget: Q4 positions inside JSON inputs');
  const offset = offsetAt(source, at);
  const token = context.semantic.getTokenAtPosition(source, offset);
  if (token.getStart(source) > offset || offset >= token.end)
    throw new Error('UnsupportedTarget: whitespace and comments have no type anchor');
  return { source, token, offset };
}

// Selection ports getTypeOfNode/isTypeDeclaration and the design's useful type
// reference anchor rule. Explicit questions are for compiler experiments only.
function selectType(context, syntax, question) {
  const { token, offset } = syntax;
  const { semantic, checker } = context;
  const { SyntaxKind: k, SymbolFlags: flags } = semantic;
  const parent = token.parent;
  const is = (node, names) => names.some(name => node?.kind === k[name]);
  let node = token;
  let symbol;
  let symbolMode;
  let demandMode;
  let kind = question;
  // `IsInExpressionContext` excludes module specifiers. A syntactic literal
  // kind alone does not authorize a Q4 expression anchor; explicit question
  // overrides remain available only as raw compiler experiments.
  if (!question && token.kind === k.StringLiteral &&
      (is(parent, ['ImportDeclaration', 'ExportDeclaration']) && parent.moduleSpecifier === token ||
       is(parent, ['ExternalModuleReference']) && parent.expression === token))
    throw new Error('UnsupportedTarget: module specifier has no Q4 typed anchor');
  const declarationName = parent?.name === token;
  const aliasName = declarationName || is(parent, ['ImportSpecifier', 'ExportSpecifier']) && parent.propertyName === token;
  if (!kind && aliasName && is(parent, ['ImportSpecifier', 'ExportSpecifier', 'ImportClause', 'NamespaceImport', 'NamespaceExport', 'ImportEqualsDeclaration'])) {
    symbol = checker.getSymbolAtLocation(token);
    if (symbol && symbol.flags & flags.Alias) symbol = checker.getAliasedSymbol(symbol);
    symbolMode = 'alias';
    if (!symbol || checker.isUnknownSymbol(symbol)) throw new Error('UnresolvedTarget: unresolved alias');
    const typeOnly = [parent, parent.parent, parent.parent?.parent, parent.parent?.parent?.parent].some(ancestor => ancestor?.isTypeOnly);
    kind = !(symbol.flags & flags.Value) || (typeOnly && symbol.flags & flags.Type) ? 'declared' : 'value';
  }
  if (!kind && declarationName && is(parent, ['TypeAliasDeclaration', 'InterfaceDeclaration', 'ClassDeclaration', 'ClassExpression', 'TypeParameter', 'EnumDeclaration'])) kind = 'declared';
  if (!kind && declarationName && is(parent, ['ModuleDeclaration'])) {
    symbol = checker.getSymbolAtLocation(token);
    if (!(symbol?.flags & flags.ValueModule)) throw new Error('UnsupportedTarget: noninstantiated namespace has no value type');
    kind = 'value';
  }
  if (!kind && declarationName && is(parent, ['VariableDeclaration', 'BindingElement', 'Parameter', 'PropertyAssignment', 'PropertyDeclaration', 'PropertySignature',
    'MethodDeclaration', 'MethodSignature', 'FunctionDeclaration', 'FunctionExpression', 'GetAccessor', 'SetAccessor', 'EnumMember'])) kind = 'value';
  if (!kind && is(parent, ['ShorthandPropertyAssignment']) && parent.name === token) {
    symbol = checker.getShorthandAssignmentValueSymbol(parent);
    symbolMode = 'shorthand';
    kind = 'value';
  }
  const parameterRole = memberCandidate(context, token)?.symbolMode;
  if (!question && ['parameter', 'parameterProperty'].includes(parameterRole)) {
    kind = 'value'; symbolMode = parameterRole;
  }
  if (!kind) {
    let entity = token;
    while (entity.parent?.kind === k.QualifiedName) entity = entity.parent;
    if (entity.parent?.kind === k.TypeQuery && entity.parent.exprName === entity) {
      // `getTypeOfNode` processes typeof operands as site-sensitive reads;
      // `getTypeOfSymbol` would bypass their native narrowing/assignment state.
      // Public question remains value, distinct from declaration-symbol reads.
      kind = 'value'; demandMode = 'location';
    }
  }
  if (!kind) {
    // The final identifier of a type reference asks about the containing type,
    // preserving its arguments; an earlier namespace segment is unsupported.
    let entity = token;
    while (entity.parent?.kind === k.QualifiedName && entity.parent.right === entity) entity = entity.parent;
    if (entity.parent?.kind === k.TypeReference && entity.parent.typeName === entity) {
      kind = 'type'; node = entity.parent;
    } else if (token.parent?.kind === k.QualifiedName) {
      throw new Error('UnsupportedTarget: earlier namespace segment of a type reference');
    } else if (semantic.isTypeNode(token)) kind = 'type';
    else if (semantic.isTypeNode(parent)) { kind = 'type'; node = parent; }
  }
  if (!kind) {
    if (parent?.kind === k.PropertyAccessExpression && parent.name === token) node = parent;
    else if (is(token, ['CloseParenToken']) && is(parent, ['CallExpression', 'NewExpression'])) node = parent;
    else if (token.kind === k.CloseBracketToken && parent?.kind === k.ElementAccessExpression) node = parent;
    else if (is(token, ['NoSubstitutionTemplateLiteral', 'TemplateTail']) && offset === token.end - 1) {
      let template = token;
      while (template.parent && !is(template, ['TaggedTemplateExpression']) && is(template.parent, ['TemplateSpan', 'TemplateExpression', 'TaggedTemplateExpression'])) template = template.parent;
      if (template.kind === k.TaggedTemplateExpression) node = template;
    } else if (token.kind === k.NewKeyword && parent?.kind === k.NewExpression) node = parent;
    if (!semantic.isExpression(node)) throw new Error(`UnsupportedTarget: ${k[token.kind]} has no typed anchor`);
    kind = 'expression';
  }
  if (!['declared', 'value', 'expression', 'type'].includes(kind)) throw new Error(`Unknown type question: ${kind}`);
  return { kind, node, symbol, symbolMode, demandMode };
}

// The design's fresh-request prelude: complete getResolvedSignature operations,
// outermost first. No whole-file semantic diagnostics substitute for this.
function prelude(context, node) {
  const { checker, semantic } = context;
  const calls = [];
  for (let current = node; current; current = current.parent)
    if ([semantic.SyntaxKind.CallExpression, semantic.SyntaxKind.NewExpression, semantic.SyntaxKind.TaggedTemplateExpression].includes(current.kind))
      calls.unshift(current);
  return calls.map(call => {
    checker.getResolvedSignature(call);
    return { kind: semantic.SyntaxKind[call.kind], location: location(context, sourceOf(call), call) };
  });
}

function establishHistory(context, history) {
  if (!['baseline', 'cold', 'diagnostics'].includes(history)) throw new Error(`Unknown checking history: ${history}`);
  if (history === 'diagnostics') context.project.program.getSemanticDiagnostics();
}

function describeType(context, type) {
  if (!type) throw new Error('TypeScript returned no type for the selected demand');
  return { summary: context.checker.typeToString(type), errorType: type.isErrorType() };
}

function describeDiagnostics(context) {
  return context.project.program.getSemanticDiagnostics().map(diagnostic => {
    const source = diagnostic.fileName && context.project.program.getSourceFile(diagnostic.fileName);
    return { code: diagnostic.code, message: diagnostic.text,
      ...(source ? { location: location(context, source, { getStart: () => diagnostic.pos, end: diagnostic.end }) } : {}) };
  });
}

/** One isolated Q4 demand; at is a one-based UTF-16 source position. */
export async function typeOracle(config, request) {
  return freshRequest(config, context => {
    const { semantic, project, checker } = context;
    const syntax = selectedSyntax(context, request.at);
    const history = request.history ?? 'baseline';
    establishHistory(context, history);
    const selected = selectType(context, syntax, request.question);
    if (['parameter', 'parameterProperty'].includes(selected.symbolMode))
      throw new Error('Unsupported public type role: parameter-property symbol pair requires the compiler hook');
    const calls = history === 'baseline' ? prelude(context, selected.node) : [];
    // Type/expression anchors can be real containers selected from synthetic
    // punctuation or keywords. Symbol observations use that established node;
    // declaration/value questions retain their actual declaration-name token.
    const symbol = selected.symbol ?? checker.getSymbolAtLocation(
      ['expression', 'type'].includes(selected.kind) ? selected.node : syntax.token);
    const type = selected.kind === 'declared' ? checker.getDeclaredTypeOfSymbol(symbol)
      : selected.kind === 'value' && selected.demandMode !== 'location' ? checker.getTypeOfSymbol(symbol) : checker.getTypeAtLocation(selected.node);
    const properties = request.properties === false || !type?.isObjectType() ? [] : checker.getPropertiesOfType(type).map(property => ({
      name: property.name, ...describeType(context, checker.getTypeOfSymbol(property)), origins: origins(context, property),
    }));
    return { compiler: semantic.version, history, kind: selected.kind, location: location(context, syntax.source, selected.node),
      sourceHash: createHash('sha256').update(syntax.source.text).digest('hex'),
      ...describeType(context, type), origins: origins(context, symbol), properties, prelude: calls,
      diagnostics: request.diagnostics === true ? describeDiagnostics(context) : [], compilerOptions: project.program.getCompilerOptions() };
  });
}

// One decoded inventory owner for getPossibleSymbolReferencePositions,
// isValidReferencePosition and getMeaningFromLocation candidate inputs. Native
// meaning/validity/relatedness remain observations from those exact operations;
// broad identifiers can be lexical nonmembers. Computed expressions are never
// relabeled with an inferred constant key. The first parameter-property modifier
// is the design's explicit member anchor, while its name keeps lexical precedence.
function memberCandidate(context, node) {
  const { SyntaxKind: k } = context.semantic;
  const parent = node.parent;
  if (!parent) return undefined;
  if (parent.kind === k.JsxNamespacedName) return undefined;
  if (node.kind === k.JsxNamespacedName && parent.kind === k.JsxAttribute && parent.name === node)
    return { key: `${node.namespace.text}:${node.name.text}`, roles: ['use'], syntax: 'jsxNamespacedAttribute', symbolMode: '' };
  const modifiers = parent.kind === k.Parameter && parent.parent?.kind === k.Constructor
    ? parent.modifiers?.filter(modifier => ['PublicKeyword', 'PrivateKeyword', 'ProtectedKeyword', 'ReadonlyKeyword', 'OverrideKeyword']
      .some(kind => modifier.kind === k[kind])) : undefined;
  if (modifiers?.[0] === node && parent.name?.kind === k.Identifier)
    return { key: parent.name.text, roles: ['declaration'], syntax: 'parameterPropertyModifier', symbolMode: 'parameterProperty' };
  const identifier = [k.Identifier, k.PrivateIdentifier].includes(node.kind);
  const literal = [k.StringLiteral, k.NoSubstitutionTemplateLiteral, k.NumericLiteral].includes(node.kind);
  if (!identifier && !literal) return undefined;
  const declared = ['PropertyDeclaration', 'PropertySignature', 'PropertyAssignment', 'ShorthandPropertyAssignment', 'MethodDeclaration',
    'MethodSignature', 'GetAccessor', 'SetAccessor', 'EnumMember', 'ModuleDeclaration'].some(kind => parent.kind === k[kind]) && parent.name === node;
  const computed = parent.kind === k.ComputedPropertyName && parent.expression === node;
  const element = parent.kind === k.ElementAccessExpression && parent.argumentExpression === node;
  const indexed = parent.kind === k.LiteralType && parent.parent?.kind === k.IndexedAccessType;
  const binding = parent.kind === k.BindingElement && parent.propertyName === node;
  // Keep every literal in the independent inventory. The Go observation calls
  // native isValidReferencePosition before any candidate semantic demand.
  if (node.kind === k.Identifier && parent.kind === k.ShorthandPropertyAssignment && parent.name === node)
    return { key: node.text, roles: ['declaration', 'use'], syntax: 'shorthandProperty', symbolMode: '' };
  if (modifiers?.length && parent.name === node)
    return { key: node.text, roles: ['declaration'], syntax: 'parameterPropertyName', symbolMode: 'parameter' };
  const declaration = parent.name === node && ![k.PropertyAccessExpression, k.JsxAttribute, k.JsxOpeningElement, k.JsxClosingElement,
    k.JsxSelfClosingElement, k.NamedTupleMember].includes(parent.kind);
  return { key: node.text, roles: declared || computed && literal || declaration ? ['declaration'] : ['use'],
    syntax: parent.kind === k.JsxAttribute ? 'jsxAttribute' : binding ? 'bindingProperty'
      : computed || element ? literal ? 'literalKey' : 'computedValue'
      : indexed ? 'indexedTypeKey' : node.kind === k.PrivateIdentifier ? 'privateName'
      : declared ? 'propertyName' : 'name', symbolMode: '' };
}

function selectMember(context, syntax) {
  // The supported member key is the whole JSX namespaced attribute. The API
  // tokenizer supplies one identifier child, so retain its actual parent node.
  const k = context.semantic.SyntaxKind;
  const node = syntax.token.parent?.kind === k.JsxNamespacedName
    && syntax.token.parent.parent?.kind === k.JsxAttribute ? syntax.token.parent : syntax.token;
  const candidate = memberCandidate(context, node);
  if (!candidate) throw new Error('UnsupportedTarget: position is not a supported reference name or member anchor');
  return { ...candidate, node, symbolMode: candidate.syntax === 'shorthandProperty' ? 'shorthand' : candidate.symbolMode };
}

function memberCandidates(context, name) {
  const result = [];
  for (const file of context.project.program.getSourceFileNames()) {
    const metadata = context.project.program.getSourceFileMetadata(file);
    if (metadata.isDefaultLibrary || metadata.isFromExternalLibrary) continue;
    const source = context.project.program.getSourceFile(file);
    const pending = [source];
    while (pending.length) {
      const node = pending.pop();
      node.forEachChild(child => { pending.push(child); });
      const candidate = memberCandidate(context, node);
      if (candidate?.key === name)
        result.push({ ...candidate, node, location: location(context, source, node), role: candidate.roles[0] });
    }
  }
  return result.sort((a, b) => compareLocations(a.location, b.location) || (a.role === b.role ? 0 : a.role === 'declaration' ? -1 : 1));
}

function compareLists(left, right, compare) {
  for (let index = 0; index < Math.min(left.length, right.length); index++) {
    const order = compare(left[index], right[index]);
    if (order) return order;
  }
  return left.length - right.length;
}
const compareOrigins = (a, b) => compareLocations(a.location, b.location) || compareExactText(a.kind, b.kind);
const compareGroups = (a, b) => compareLists(a.origins, b.origins, compareOrigins) || compareLists(a.occurrences, b.occurrences, compareLocations);

function combinedMember(context, symbol, token) {
  const { SyntaxKind: k, SymbolFlags: flags } = context.semantic;
  const parent = symbol?.getParent();
  if (parent?.flags & (flags.Class | flags.Interface)) {
    const declaration = token.parent;
    const isStatic = declaration?.modifiers?.some(modifier => modifier.kind === k.StaticKeyword);
    const type = isStatic ? context.checker.getTypeOfSymbol(parent) : context.checker.getDeclaredTypeOfSymbol(parent);
    return context.checker.getPropertyOfType(type, symbol.name) ?? symbol;
  }
  if (token.parent?.kind === k.PropertyAccessExpression) {
    const receiver = context.checker.getTypeAtLocation(token.parent.expression);
    return receiver ? context.checker.getPropertyOfType(receiver, symbol.name) ?? symbol : symbol;
  }
  return symbol;
}

/**
 * Records API seed/candidate demands and LS reference observations separately.
 * The public API's LS references acquire another checker. They cannot prove the
 * design's shared-state Q3 schedule; callers must not label this result complete.
 */
export async function memberOracle(config, request) {
  return freshRequest(config, context => {
    const syntax = selectedSyntax(context, request.at, true);
    const selected = selectMember(context, syntax);
    if (['parameter', 'parameterProperty'].includes(selected.symbolMode))
      throw new Error('Unsupported public member role: parameter-property symbol pair requires the compiler hook');
    const history = request.history ?? 'baseline';
    establishHistory(context, history);
    const calls = history === 'baseline' ? prelude(context, selected.node) : [];
    const rawSymbol = selected.symbolMode === 'shorthand' ? context.checker.getShorthandAssignmentValueSymbol(selected.node.parent)
      : context.checker.getSymbolAtLocation(selected.node);
    if (!rawSymbol || context.checker.isUnknownSymbol(rawSymbol)) throw new Error('UnresolvedTarget: no member symbol');
    const rawOrigins = origins(context, rawSymbol);
    const combined = combinedMember(context, rawSymbol, selected.node);
    const combinedOrigins = origins(context, combined);
    const candidates = memberCandidates(context, combined.name).map(candidate => {
      const sitePrelude = history === 'baseline' ? prelude(context, candidate.node) : [];
      const symbol = context.checker.getSymbolAtLocation(candidate.node);
      return { location: candidate.location, role: candidate.role, origins: origins(context, symbol), prelude: sitePrelude };
    });
    const referenceGroups = context.checker.getReferencedSymbolsForNode(selected.node, syntax.offset).map(group => ({
      origins: origins(context, group.symbol),
      occurrences: group.references.map(handle => {
        const node = handle.resolve(context.project);
        if (!node) throw new Error(`Cannot resolve reference in ${handle.path}`);
        return location(context, sourceOf(node), node);
      }).sort(compareLocations),
    }));
    return { compiler: context.semantic.version, history, seed: syntax.token.text, prelude: calls, rawOrigins, combinedOrigins, candidates,
      rawReferenceGroups: referenceGroups,
      normalization: { kind: 'combinedMemberOrigins', status: 'observed', sharedCheckerSchedule: 'unproved' },
      blockers: ['public_reference_api_uses_separate_checker'], diagnostics: request.diagnostics === true ? describeDiagnostics(context) : [] };
  });
}

const GO_SOURCE_COMMIT = '2bd066d87f5bafd315be9f40889d0a60b9e58e0b';
const hookFixtures = new URL('../fixtures/types/compiler-hook/', import.meta.url);

function runProcess(binary, args, options = {}) {
  const result = spawnSync(binary, args, { encoding: 'utf8', timeout: 180_000, maxBuffer: 10 * 1024 * 1024, ...options });
  if (result.error || result.signal || result.status !== 0)
    throw new Error(`Compiler hook ${binary} failed: ${result.error?.message || result.stderr || result.stdout || `exit ${result.status}, signal ${result.signal}`}`);
  return result.stdout.trim();
}

function pinnedGoSource() {
  const common = runProcess('git', ['rev-parse', '--path-format=absolute', '--git-common-dir']);
  const directory = process.env.TYPE_SCRIPT_GO_SOURCE || path.join(path.dirname(common), '.cache/typescript-go-7.0.2');
  if (runProcess('git', ['rev-parse', 'HEAD'], { cwd: directory }) !== GO_SOURCE_COMMIT)
    throw new Error(`TYPE_SCRIPT_GO_SOURCE must be pinned at ${GO_SOURCE_COMMIT}`);
  if (runProcess('git', ['rev-parse', 'typescript/v7.0.2^{commit}'], { cwd: directory }) !== GO_SOURCE_COMMIT)
    throw new Error('Compiler source is missing the pinned typescript/v7.0.2 tag');
  runProcess('git', ['diff', '--quiet', '--no-ext-diff', 'HEAD', '--'], { cwd: directory });
  const untracked = runProcess('git', ['ls-files', '--others', '--exclude-standard'], { cwd: directory });
  const ignoredBuildInputs = runProcess('git', ['ls-files', '--others', '--ignored', '--exclude-standard', '--', 'internal/**/*.go', 'cmd/**/*.go', 'go.mod', 'go.sum'], { cwd: directory });
  if (untracked || ignoredBuildInputs) throw new Error(`Pinned compiler source contains untracked build inputs: ${untracked || ignoredBuildInputs}`);
  return directory;
}

function replaceExactly(source, before, after) {
  if (source.split(before).length !== 2) throw new Error(`Pinned compiler hook anchor differs: ${before.trim()}`);
  return source.replace(before, after);
}

/**
 * Executes a test-only overlay of the exact Go 7.0.2 source. The overlay exposes
 * private compiler operations/counters and inserts candidate checking after the
 * actual relationship owner constructs its search set. Source files are never
 * changed. Missing Go/source/version/build/output errors fail the request.
 * One native checker owns the observation stream. The overlay fixes only the
 * pool-size performance option after verifying captured compiler options; native
 * binder scheduling remains controlled separately by singleThreadBinding.
 * `currentDirectory` controls the compiler host independently of the config
 * directory, which remains the base for request and result locations.
 */
export async function compilerHookOracle(config, request) {
  const operation = request.operation ?? 'type';
  if (!['type', 'member'].includes(operation)) throw new Error(`Unknown oracle operation: ${operation}`);
  const history = request.history ?? 'baseline';
  if (!['baseline', 'cold', 'diagnostics'].includes(history)) throw new Error(`Unknown checking history: ${history}`);
  if (history === 'diagnostics') throw new Error('Unsupported compiler hook history: diagnostics-first checker equivalence remains unproved');
  const sourceDirectory = pinnedGoSource();
  let capturedInputs;
  const input = await freshRequest(config, context => {
    capturedInputs = context.capturedInputs;
    const syntax = selectedSyntax(context, request.at, operation === 'member');
    const selected = operation === 'member' ? selectMember(context, syntax) : request.inspectSymbolDisplay
      ? { kind: 'symbol', node: (() => {
        for (let node = syntax.token; node; node = node.parent) {
          const name = node.name;
          if (name && name.getStart(syntax.source) <= syntax.offset && syntax.offset < name.end) return name;
        }
        return syntax.token;
      })() } : selectType(context, syntax, request.question);
    const sourceFiles = context.project.program.getSourceFileNames();
    const programFiles = sourceFiles.map(file => {
      const metadata = context.project.program.getSourceFileMetadata(file);
      return { file, defaultLibrary: metadata.isDefaultLibrary, externalLibrary: metadata.isFromExternalLibrary };
    });
    const files = context.capturedInputs.files;
    // Captured inputs retain their original UTF-8 bytes, including a BOM. Only
    // virtual sources absent from readFile need the API's decoded source text.
    for (const file of sourceFiles) if (!Object.hasOwn(files, file)) files[file] = context.project.program.getSourceFile(file).text;
    const candidates = operation === 'member' ? memberCandidates(context, selected.key) : [];
    const byteOffset = (source, offset) => Buffer.byteLength(source.text.slice(0, offset), 'utf8');
    return { files, root: context.root, currentDirectory: context.currentDirectory, config: context.configPath, file: syntax.source.fileName,
      offset: byteOffset(syntax.source, syntax.offset), operation,
      question: selected?.kind ?? '', symbolMode: selected?.symbolMode ?? '', demandMode: selected?.demandMode ?? '', normalize: history === 'baseline',
      inspectArrayElement: request.inspectArrayElement === true,
      inspectSummaryBytes: request.inspectSummaryBytes === true,
      noTruncation: request.noTruncation === true,
      inspectDiagnosticCache: request.inspectDiagnosticCache === true,
      inspectExpressionAcquisitions: request.inspectExpressionAcquisitions === true,
      inspectSymbolDisplay: request.inspectSymbolDisplay === true,
      symbolDisplayEnclosing: request.symbolDisplayEnclosing === true,
      symbolIDPrefix: request.symbolIDPrefix ?? 0,
      singleThreadBinding: request.singleThreadBinding === true,
      inspectPrivateNames: request.inspectPrivateNames === true,
      comparePrototypeSymbols: request.comparePrototypeSymbols === true,
      observeRightPrototypeFirst: request.observeRightPrototypeFirst === true,
      fileSystem: context.capturedInputs.answers, caseSensitive: context.caseSensitive,
      sourceFiles: programFiles.filter(file => !file.defaultLibrary).map(file => file.file),
      programFiles,
      candidateLedger: candidates.map(({ node, role, symbolMode, ...candidate }) => candidate),
      nodeKind: selected?.node?.kind ?? syntax.token.kind,
      selection: operation === 'member' ? { key: selected.key, roles: selected.roles, syntax: selected.syntax } : undefined,
      nodeStart: byteOffset(syntax.source, selected?.node?.getStart(syntax.source) ?? syntax.token.getStart(syntax.source)),
      nodeEnd: byteOffset(syntax.source, selected?.node?.end ?? syntax.token.end),
      candidates: candidates.map(candidate => ({ file: sourceOf(candidate.node).fileName,
        offset: byteOffset(sourceOf(candidate.node), candidate.node.getStart(sourceOf(candidate.node))), symbolMode: candidate.symbolMode,
        nodeKind: candidate.node.kind, nodeStart: byteOffset(sourceOf(candidate.node), candidate.node.getStart(sourceOf(candidate.node))),
        nodeEnd: byteOffset(sourceOf(candidate.node), candidate.node.end) })),
      compilerOptions: context.project.program.getCompilerOptions(), syntaxKind: context.semantic.SyntaxKind };
  }, true, request.currentDirectory);
  const temporary = fs.mkdtempSync(path.resolve('.cc/temp/type-oracle-hook-'));
  try {
    const replace = {};
    const attach = (source, destination) => { replace[path.join(sourceDirectory, destination)] = source; };
    attach(fileURLToPath(new URL('checker-hook.go', hookFixtures)), 'internal/checker/zz_agent_oracle_hook.go');
    attach(fileURLToPath(new URL('ls-hook.go', hookFixtures)), 'internal/ls/zz_agent_oracle_hook.go');
    attach(fileURLToPath(new URL('oracle-test.go', hookFixtures)), 'internal/ls/zz_agent_oracle_test.go');
    const checkerPath = path.join(sourceDirectory, 'internal/checker/checker.go');
    let checkerSource = fs.readFileSync(checkerPath, 'utf8');
    if (request.inspectAwaitContexts === true) {
      // Observe only values acquired by the native operation. In particular,
      // absent context and nil awaited results must not force the next getter.
      const declaration = 'func (c *Checker) getContextualTypeForAwaitOperand(node *ast.Node, contextFlags ContextFlags) *Type {';
      const start = checkerSource.indexOf(declaration);
      if (start < 0) throw new Error('Pinned await contextual owner is absent');
      const end = checkerSource.indexOf('\n}', start)+2;
      if (end < start) throw new Error('Pinned await contextual owner has no end');
      const original = checkerSource.slice(start, end);
      let observed = replaceExactly(original,
        '\tcontextualType := c.getContextualType(node, contextFlags)',
        '\tcontextualType := c.getContextualType(node, contextFlags)\n\tAgentOracleObservation("awaitContext", map[string]any{"nil": contextualType == nil})');
      observed = replaceExactly(observed,
        '\t\tcontextualAwaitedType := c.getAwaitedTypeNoAlias(contextualType)',
        '\t\tcontextualAwaitedType := c.getAwaitedTypeNoAlias(contextualType)\n\t\tAgentOracleObservation("awaitContextResult", map[string]any{"nil": contextualAwaitedType == nil})');
      checkerSource = replaceExactly(checkerSource, original, observed);
      checkerSource = replaceExactly(checkerSource,
        '\tglobalPromiseLikeType := c.getGlobalPromiseLikeType()',
        '\tglobalPromiseLikeType := c.getGlobalPromiseLikeType()\n\tAgentOracleObservation("promiseLikeFactory", map[string]any{"emptyRecovery": globalPromiseLikeType == c.emptyGenericType})');
    }
    // Optional conditional-cache evidence observes existing pointers/slots and
    // already-emitted diagnostics/acquisition events. It adds no semantic calls.
    if (request.inspectConditionalCaches === true) {
      checkerSource = replaceExactly(checkerSource,
        '\t\tif tailCount == 1000 {\n\t\t\tc.error(c.currentNode, diagnostics.Type_instantiation_is_excessively_deep_and_possibly_infinite)',
        '\t\tif tailCount == 1000 {\n\t\t\tif c.currentNode != nil { AgentOracleObservation("conditionalTailLocation", map[string]any{"kind": c.currentNode.Kind, "pos": c.currentNode.Pos(), "end": c.currentNode.End()}) }\n\t\t\tc.error(c.currentNode, diagnostics.Type_instantiation_is_excessively_deep_and_possibly_infinite)');
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getRestrictiveInstantiation(t *Type) *Type {',
        `func (c *Checker) getRestrictiveInstantiation(t *Type) (agentResult *Type) {
    agentKey := CachedTypeKey{kind: CachedTypeKindRestrictiveInstantiation, typeId: t.id}
    agentCached := c.cachedTypes[agentKey]
    agentAcquisitionStart := len(AgentOracleDisplayAcquisitions)
    agentDiagnosticStart := len(c.AgentOracleCurrentDiagnostics())
    AgentOracleCounters["restrictiveInvocation"]++
    agentInvocation := AgentOracleCounters["restrictiveInvocation"]
    agentNode := c.currentNode
    defer func() {
        errors := 0
        for _, event := range AgentOracleDisplayAcquisitions[agentAcquisitionStart:] {
            if event["event"] == "exit" && event["errorType"] == true { errors++ }
        }
        row := map[string]any{
            "input": t.id, "flags": uint32(t.flags), "same": agentResult == t,
            "cacheHit": agentCached != nil, "cacheSelfBefore": agentCached == t,
            "cacheSelfAfter": c.cachedTypes[agentKey] == t,
            "acquisitionStart": agentAcquisitionStart, "acquisitionEnd": len(AgentOracleDisplayAcquisitions),
            "errorAcquisitions": errors, "diagnosticsAdded": len(c.AgentOracleCurrentDiagnostics()) - agentDiagnosticStart,
        }
        if agentResult != nil { row["result"] = agentResult.id }
        if agentNode != nil { row["kind"], row["pos"], row["end"] = agentNode.Kind, agentNode.Pos(), agentNode.End() }
        AgentOracleObservation(fmt.Sprintf("restrictive:%d", agentInvocation), row)
    }()`);
    }
    // Private-access proof reads existing values only. The self-access
    // expression and both lexical table lookups still execute exactly once.
    checkerSource = replaceExactly(checkerSource,
      "func (c *Checker) markLinkedReferences(location *ast.Node, hint ReferenceHint, propSymbol *ast.Symbol, parentType *Type) {",
      "func (c *Checker) markLinkedReferences(location *ast.Node, hint ReferenceHint, propSymbol *ast.Symbol, parentType *Type) {\n\tif hint == ReferenceHintProperty {\n\t\tAgentOracleCounters[\"privateAccess\"]++\n\t\tAgentOracleObservation(fmt.Sprintf(\"privateAccess:%d\", AgentOracleCounters[\"privateAccess\"]), map[string]any{\"event\": \"linked\", \"enabled\": c.canCollectSymbolAliasAccessibilityData, \"ambient\": location.Flags&ast.NodeFlagsAmbient != 0, \"property\": propSymbol != nil, \"parentType\": parentType != nil, \"pos\": location.Pos(), \"end\": location.End()})\n\t}");
    checkerSource = replaceExactly(checkerSource,
      "func (c *Checker) isSelfTypeAccess(name *ast.Node, parent *ast.Symbol) bool {\n\treturn name.Kind == ast.KindThisKeyword || parent != nil && ast.IsEntityNameExpression(name) && parent == c.getResolvedSymbol(ast.GetFirstIdentifier(name))\n}",
      "func (c *Checker) isSelfTypeAccess(name *ast.Node, parent *ast.Symbol) bool {\n\tresult := name.Kind == ast.KindThisKeyword || parent != nil && ast.IsEntityNameExpression(name) && parent == c.getResolvedSymbol(ast.GetFirstIdentifier(name))\n\tAgentOracleCounters[\"privateAccess\"]++\n\tAgentOracleObservation(fmt.Sprintf(\"privateAccess:%d\", AgentOracleCounters[\"privateAccess\"]), map[string]any{\"event\": \"self\", \"kind\": name.Kind, \"parent\": parent != nil, \"result\": result, \"pos\": name.Pos(), \"end\": name.End()})\n\treturn result\n}");
    checkerSource = replaceExactly(checkerSource,
      "\t\tprop := symbol.Members[name]\n\t\tif prop != nil {\n\t\t\treturn prop\n\t\t}\n\t\tprop = symbol.Exports[name]\n\t\tif prop != nil {\n\t\t\treturn prop\n\t\t}",
      "\t\tprop := symbol.Members[name]\n\t\tAgentOracleCounters[\"privateAccess\"]++\n\t\tAgentOracleObservation(fmt.Sprintf(\"privateAccess:%d\", AgentOracleCounters[\"privateAccess\"]), map[string]any{\"event\": \"members\", \"found\": prop != nil, \"classPos\": containingClass.Pos(), \"classEnd\": containingClass.End(), \"pos\": location.Pos(), \"end\": location.End()})\n\t\tif prop != nil {\n\t\t\treturn prop\n\t\t}\n\t\tprop = symbol.Exports[name]\n\t\tAgentOracleCounters[\"privateAccess\"]++\n\t\tAgentOracleObservation(fmt.Sprintf(\"privateAccess:%d\", AgentOracleCounters[\"privateAccess\"]), map[string]any{\"event\": \"exports\", \"found\": prop != nil, \"classPos\": containingClass.Pos(), \"classEnd\": containingClass.End(), \"pos\": location.Pos(), \"end\": location.End()})\n\t\tif prop != nil {\n\t\t\treturn prop\n\t\t}");
    checkerSource = replaceExactly(checkerSource,
      "func (c *Checker) checkExternalEmitHelpers(location *ast.Node, helpers ExternalEmitHelpers) {",
      "func (c *Checker) checkExternalEmitHelpers(location *ast.Node, helpers ExternalEmitHelpers) {\n\tAgentOracleCounters[\"privateAccess\"]++\n\tAgentOracleObservation(fmt.Sprintf(\"privateAccess:%d\", AgentOracleCounters[\"privateAccess\"]), map[string]any{\"event\": \"helpers\", \"helpers\": helpers, \"enabled\": c.compilerOptions.ImportHelpers.IsTrue(), \"pos\": location.Pos(), \"end\": location.End()})");
    checkerSource = replaceExactly(checkerSource,
      "func (c *Checker) markAliasSymbolAsReferenced(symbol *ast.Symbol) {\n\tlinks := c.aliasSymbolLinks.Get(symbol)",
      "func (c *Checker) markAliasSymbolAsReferenced(symbol *ast.Symbol) {\n\tlinks := c.aliasSymbolLinks.Get(symbol)\n\tAgentOracleCounters[\"privateAccess\"]++\n\tAgentOracleObservation(fmt.Sprintf(\"privateAccess:%d\", AgentOracleCounters[\"privateAccess\"]), map[string]any{\"event\": \"alias\", \"name\": symbol.Name, \"alreadyReferenced\": links.referenced})");
    // Observe raw declaration headers only at existing native consumers.
    // No getMergedSymbol, symbol-ID allocation, or type getter is added.
    checkerSource = replaceExactly(checkerSource,
      'func (c *Checker) getSymbolOfDeclaration(node *ast.Node) *ast.Symbol {\n\tsymbol := node.Symbol()',
      'func (c *Checker) getSymbolOfDeclaration(node *ast.Node) *ast.Symbol {\n\tsymbol := node.Symbol()\n\tif symbol != nil && symbol.Name == "globalThis" {\n\t\tmerged := c.mergedSymbols[symbol]\n\t\tAgentOracleObservation(fmt.Sprintf("globalThisDeclaration:%d:%d", node.Pos(), node.End()), map[string]any{"kind": node.Kind, "pos": node.Pos(), "end": node.End(), "rawIsGlobal": symbol == c.globalThisSymbol, "mappedToGlobal": merged == c.globalThisSymbol, "rawDeclarations": len(symbol.Declarations), "globalDeclarations": len(c.globalThisSymbol.Declarations)})\n\t}');
    checkerSource = replaceExactly(checkerSource,
      '\t\tdeclarations := failed.declaration.Symbol().Declarations\n\t\tif len(declarations) > 1 {',
      '\t\tdeclarations := failed.declaration.Symbol().Declarations\n\t\tif failed.declaration.Symbol().Name == "globalThis" {\n\t\t\traw := failed.declaration.Symbol()\n\t\t\tAgentOracleObservation(fmt.Sprintf("globalThisRawImplementation:%d:%d", failed.declaration.Pos(), failed.declaration.End()), map[string]any{"kind": failed.declaration.Kind, "pos": failed.declaration.Pos(), "end": failed.declaration.End(), "rawIsGlobal": raw == c.globalThisSymbol, "mappedToGlobal": c.mergedSymbols[raw] == c.globalThisSymbol, "rawDeclarations": len(declarations), "globalDeclarations": len(c.globalThisSymbol.Declarations)})\n\t\t}\n\t\tif len(declarations) > 1 {');
    checkerSource = replaceExactly(checkerSource,
      'func (c *Checker) getTypeOfSymbol(symbol *ast.Symbol) *Type {',
      'func (c *Checker) getTypeOfSymbol(symbol *ast.Symbol) *Type {\n\tif symbol == c.globalThisSymbol {\n\t\tAgentOracleObservation("globalThisValue", map[string]any{"initialized": true, "declarations": len(symbol.Declarations), "transient": symbol.Flags&ast.SymbolFlagsTransient != 0, "readonly": symbol.CheckFlags&ast.CheckFlagsReadonly != 0})\n\t}');
    // Declaration-check proof reads only fields at the actual native branch.
    checkerSource = replaceExactly(checkerSource,
      "func (c *Checker) checkVariableLikeDeclaration(node *ast.Node) {\n\tc.checkDecorators(node)",
      "func (c *Checker) checkVariableLikeDeclaration(node *ast.Node) {\n\tAgentOracleCounters[\"variableLikeInvocation\"]++\n\tAgentOracleObservation(fmt.Sprintf(\"variableLikeEntry:%d\", AgentOracleCounters[\"variableLikeInvocation\"]), map[string]any{\"kind\": node.Kind, \"pos\": node.Pos(), \"end\": node.End()})\n\tc.checkDecorators(node)");
    checkerSource = replaceExactly(checkerSource,
      "\t\tneedCheckWidenedType := !core.Some(name.Elements(), func(n *ast.Node) bool { return n.Name() != nil })\n",
      "\t\tneedCheckWidenedType := !core.Some(name.Elements(), func(n *ast.Node) bool { return n.Name() != nil })\n\t\tAgentOracleObservation(fmt.Sprintf(\"variableLikePattern:%d:%d\", node.Pos(), node.End()), map[string]any{\"initializer\": needCheckInitializer, \"empty\": needCheckWidenedType, \"kind\": name.Kind})\n");
    checkerSource = replaceExactly(checkerSource,
      "\t// For a commonjs `const x = require`, validate the alias and exit\n\tsymbol := c.getSymbolOfDeclaration(node)\n",
      "\t// For a commonjs `const x = require`, validate the alias and exit\n\tsymbol := c.getSymbolOfDeclaration(node)\n\tAgentOracleObservation(fmt.Sprintf(\"variableLikeSymbol:%d:%d\", node.Pos(), node.End()), map[string]any{\"primary\": node == symbol.ValueDeclaration, \"kind\": node.Kind})\n");
    checkerSource = replaceExactly(checkerSource,
      "\tif initializer != nil && ast.IsPartOfParameterDeclaration(node) && ast.NodeIsMissing(ast.GetContainingFunction(node).Body()) {\n\t\tc.error(node, diagnostics.A_parameter_initializer_is_only_allowed_in_a_function_or_constructor_implementation)",
      "\tif initializer != nil && ast.IsPartOfParameterDeclaration(node) && ast.NodeIsMissing(ast.GetContainingFunction(node).Body()) {\n\t\tAgentOracleObservation(fmt.Sprintf(\"variableLikeBodyless:%d:%d\", node.Pos(), node.End()), map[string]any{\"bodyless\": true})\n\t\tc.error(node, diagnostics.A_parameter_initializer_is_only_allowed_in_a_function_or_constructor_implementation)");
    // Observe only the existing maybeTypeOfKind recursion and short circuit.
    // This opt-in overlay does not acquire any new semantic type or symbol.
    if (request.inspectKindPredicates === true) {
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) maybeTypeOfKind(t *Type, kind TypeFlags) bool {',
        `func (c *Checker) maybeTypeOfKind(t *Type, kind TypeFlags) (result bool) {
    AgentOracleCounters["kindPredicateInvocation"]++
    invocation := AgentOracleCounters["kindPredicateInvocation"]
    AgentOracleCounters["kindPredicateDepth"]++
    depth := AgentOracleCounters["kindPredicateDepth"]
    visited := 0
    count := 0
    if t.flags&TypeFlagsUnionOrIntersection != 0 { count = len(t.Types()) }
    defer func() {
        AgentOracleObservation(fmt.Sprintf("maybeTypeOfKind:%d", invocation), map[string]any{
            "input": t.id, "flags": uint32(t.flags), "kind": uint32(kind),
            "count": count, "visited": visited, "depth": depth, "result": result,
        })
        AgentOracleCounters["kindPredicateDepth"]--
    }()`);
      checkerSource = replaceExactly(checkerSource,
        '\t\tfor _, t := range t.Types() {\n\t\t\tif c.maybeTypeOfKind(t, kind) {',
        '\t\tfor _, t := range t.Types() {\n\t\t\tvisited++\n\t\t\tif c.maybeTypeOfKind(t, kind) {');
    }
    // Zero-demand filterType branch evidence: the original two predicate
    // passes and native factories are unchanged. Observe only existing fields.
    checkerSource = replaceExactly(checkerSource,
      "func (c *Checker) filterType(t *Type, f func(*Type) bool) *Type {\n\tif t.flags&TypeFlagsUnion != 0 {\n\t\ttypes := t.Types()\n\t\tfiltered := core.Filter(types, f)\n\t\tif core.Same(types, filtered) {\n\t\t\treturn t\n\t\t}\n\t\torigin := t.AsUnionType().origin\n\t\tvar newOrigin *Type\n\t\tif origin != nil && origin.flags&TypeFlagsUnion != 0 {\n\t\t\t// If the origin type is a (denormalized) union type, filter its non-union constituents. If that ends\n\t\t\t// up removing a smaller number of types than in the normalized constituent set (meaning some of the\n\t\t\t// filtered types are within nested unions in the origin), then we can't construct a new origin type.\n\t\t\t// Otherwise, if we have exactly one type left in the origin set, return that as the filtered type.\n\t\t\t// Otherwise, construct a new filtered origin type.\n\t\t\toriginTypes := origin.Types()\n\t\t\toriginFiltered := core.Filter(originTypes, func(u *Type) bool {\n\t\t\t\treturn u.flags&TypeFlagsUnion != 0 || f(u)\n\t\t\t})\n\t\t\tif len(originTypes)-len(originFiltered) == len(types)-len(filtered) {\n\t\t\t\tif len(originFiltered) == 1 {\n\t\t\t\t\treturn originFiltered[0]\n\t\t\t\t}\n\t\t\t\tnewOrigin = c.newUnionType(ObjectFlagsNone, originFiltered)\n\t\t\t}\n\t\t}\n\t\t// filtering could remove intersections so `ContainsIntersections` might be forwarded \"incorrectly\"\n\t\t// it is purely an optimization hint so there is no harm in accidentally forwarding it\n\t\treturn c.getUnionTypeFromSortedList(filtered, t.AsUnionType().objectFlags&(ObjectFlagsPrimitiveUnion|ObjectFlagsContainsIntersections), nil /*alias*/, newOrigin)\n\t}\n\tif t.flags&TypeFlagsNever != 0 || f(t) {\n\t\treturn t\n\t}\n\treturn c.neverType\n}",
      "func (c *Checker) filterType(t *Type, f func(*Type) bool) (result *Type) {\n\tAgentOracleCounters[\"filterTypeInvocation\"]++\n\tinvocation := AgentOracleCounters[\"filterTypeInvocation\"]\n\tbranch, originKind := \"scalar-kept\", \"none\"\n\tnormalizedCount, normalizedRemoved, originCount, originRemoved, originPredicates := 1, 0, 0, 0, 0\n\tdefer func() {\n\t\tAgentOracleObservation(fmt.Sprintf(\"filterType:%d\", invocation), map[string]any{\n\t\t\t\"branch\": branch, \"originKind\": originKind, \"normalizedPredicates\": normalizedCount,\n\t\t\t\"normalizedRemoved\": normalizedRemoved, \"originCount\": originCount,\n\t\t\t\"originRemoved\": originRemoved, \"originPredicates\": originPredicates,\n\t\t\t\"sameInput\": result == t,\n\t\t})\n\t}()\n\tif t.flags&TypeFlagsUnion != 0 {\n\t\ttypes := t.Types()\n\t\tnormalizedCount = len(types)\n\t\tfiltered := core.Filter(types, f)\n\t\tnormalizedRemoved = len(types) - len(filtered)\n\t\tif core.Same(types, filtered) {\n\t\t\tbranch = \"union-unchanged\"\n\t\t\treturn t\n\t\t}\n\t\tbranch = \"origin-discarded\"\n\t\torigin := t.AsUnionType().origin\n\t\tvar newOrigin *Type\n\t\tif origin != nil {\n\t\t\toriginKind = \"other\"\n\t\t\tif origin.flags&TypeFlagsIntersection != 0 { originKind = \"intersection\" }\n\t\t}\n\t\tif origin != nil && origin.flags&TypeFlagsUnion != 0 {\n\t\t\toriginKind = \"union\"\n\t\t\toriginTypes := origin.Types()\n\t\t\toriginCount = len(originTypes)\n\t\t\toriginFiltered := core.Filter(originTypes, func(u *Type) bool {\n\t\t\t\tif u.flags&TypeFlagsUnion == 0 { originPredicates++ }\n\t\t\t\treturn u.flags&TypeFlagsUnion != 0 || f(u)\n\t\t\t})\n\t\t\toriginRemoved = len(originTypes) - len(originFiltered)\n\t\t\tif len(originTypes)-len(originFiltered) == len(types)-len(filtered) {\n\t\t\t\tif len(originFiltered) == 1 {\n\t\t\t\t\tbranch = \"origin-singleton\"\n\t\t\t\t\treturn originFiltered[0]\n\t\t\t\t}\n\t\t\t\tbranch = \"origin-rebuilt\"\n\t\t\t\tnewOrigin = c.newUnionType(ObjectFlagsNone, originFiltered)\n\t\t\t}\n\t\t}\n\t\treturn c.getUnionTypeFromSortedList(filtered, t.AsUnionType().objectFlags&(ObjectFlagsPrimitiveUnion|ObjectFlagsContainsIntersections), nil /*alias*/, newOrigin)\n\t}\n\tif t.flags&TypeFlagsNever != 0 {\n\t\tbranch = \"never\"\n\t\tnormalizedCount = 0\n\t\treturn t\n\t}\n\tif f(t) {\n\t\treturn t\n\t}\n\tbranch = \"scalar-removed\"\n\tnormalizedRemoved = 1\n\treturn c.neverType\n}");
    for (const [operation, parameters, node, symbol] of [
      ['getTypeFromTypeNode', 'node *ast.Node', 'node', 'nil'],
      ['getTypeOfSymbol', 'symbol *ast.Symbol', 'nil', 'symbol'],
    ]) {
      checkerSource = replaceExactly(checkerSource,
        `func (c *Checker) ${operation}(${parameters}) *Type {`,
        `func (c *Checker) ${operation}(${parameters}) (agentResult *Type) {\n\tif observe := c.agentOracleDisplayAcquisition("${operation}", ${node}, ${symbol}); observe != nil { defer func() { observe(agentResult) }() }`);
    }

    // Observe completed expression results after native cleanup, including
    // context-free cache hits. This opt-in trace performs no additional demand.
    for (const [operation, parameters] of [
      ['checkExpressionEx', 'node *ast.Node, checkMode CheckMode'],
      ['getContextFreeTypeOfExpression', 'node *ast.Node'],
    ]) {
      checkerSource = replaceExactly(checkerSource,
        `func (c *Checker) ${operation}(${parameters}) *Type {`,
        `func (c *Checker) ${operation}(${parameters}) (agentResult *Type) {\n\tif AgentOracleInspectExpressionAcquisitions {\n\t\tif observe := c.agentOracleDisplayAcquisition("${operation}", node, nil); observe != nil { defer func() { observe(agentResult) }() }\n\t}`);
    }

    checkerSource = replaceExactly(checkerSource,
      "func (c *Checker) checkIteratedTypeOrElementType(use IterationUse, inputType *Type, sentType *Type, errorNode *ast.Node) *Type {\n\tif IsTypeAny(inputType) {\n\t\treturn inputType\n\t}\n\tt := c.getIteratedTypeOrElementType(use, inputType, sentType, errorNode, true /*checkAssignability*/)\n\tif t != nil {\n\t\treturn t\n\t}\n\treturn c.anyType\n}",
      "func (c *Checker) checkIteratedTypeOrElementType(use IterationUse, inputType *Type, sentType *Type, errorNode *ast.Node) *Type {\n\tpos, end := -1, -1\n\tif errorNode != nil { pos, end = errorNode.Pos(), errorNode.End() }\n\tkey := fmt.Sprintf(\"checkedIteration:%d:%d:%d\", pos, end, use)\n\tif IsTypeAny(inputType) {\n\t\tAgentOracleObservation(key, map[string]any{\"inputAny\": true, \"fallbackAny\": false, \"pos\": pos, \"end\": end, \"use\": use})\n\t\treturn inputType\n\t}\n\tt := c.getIteratedTypeOrElementType(use, inputType, sentType, errorNode, true /*checkAssignability*/)\n\tAgentOracleObservation(key, map[string]any{\"inputAny\": false, \"fallbackAny\": t == nil, \"pos\": pos, \"end\": end, \"use\": use})\n\tif t != nil {\n\t\treturn t\n\t}\n\treturn c.anyType\n}");
    checkerSource = replaceExactly(checkerSource,
      "func (c *Checker) getAsyncFromSyncIterationTypes(iterationTypes IterationTypes, errorNode *ast.Node) IterationTypes {",
      "func (c *Checker) getAsyncFromSyncIterationTypes(iterationTypes IterationTypes, errorNode *ast.Node) (result IterationTypes) {\n\tdefer func() {\n\t\tAgentOracleObservation(\"asyncFromSyncIteration\", map[string]any{\"inputYieldNil\": iterationTypes.yieldType == nil, \"inputReturnNil\": iterationTypes.returnType == nil, \"inputYieldAny\": iterationTypes.yieldType == c.anyType, \"inputReturnAny\": iterationTypes.returnType == c.anyType, \"yieldAny\": result.yieldType == c.anyType, \"returnAny\": result.returnType == c.anyType, \"yieldNil\": result.yieldType == nil, \"returnNil\": result.returnType == nil})\n\t}()");
    checkerSource = replaceExactly(checkerSource,
      "\treturn IterationTypes{yieldType, c.getUnionType(returnTypes), nextType}\n}",
      "\tresult := IterationTypes{yieldType, c.getUnionType(returnTypes), nextType}\n\tAgentOracleObservation(\"iteratorMethod:\" + methodName, map[string]any{\"yieldNil\": result.yieldType == nil, \"returnNil\": result.returnType == nil, \"nextNil\": result.nextType == nil, \"yieldAny\": result.yieldType == c.anyType, \"returnAny\": result.returnType == c.anyType, \"returnCount\": len(returnTypes)})\n\treturn result\n}");
    checkerSource = replaceExactly(checkerSource,
      "\tif !iterationTypes.hasTypes() {\n\t\tif errorNode != nil {\n\t\t\tc.reportDiagnostic(NewDiagnosticForNode(errorNode, resolver.mustHaveAValueDiagnostic, methodName), diagnosticOutput)\n\t\t}\n\t\tyieldType = c.anyType",
      "\tif !iterationTypes.hasTypes() {\n\t\tAgentOracleObservation(\"iteratorMissingValue:\" + methodName, map[string]any{\"inputFlags\": uint32(resolvedMethodReturnType.flags), \"fallbackAny\": true})\n\t\tif errorNode != nil {\n\t\t\tc.reportDiagnostic(NewDiagnosticForNode(errorNode, resolver.mustHaveAValueDiagnostic, methodName), diagnosticOutput)\n\t\t}\n\t\tyieldType = c.anyType");
    checkerSource = replaceExactly(checkerSource,
      "\treturn IterationTypes{\n\t\tyieldType:  core.OrElse(c.getAwaitedTypeEx(iterationTypes.yieldType, errorNode, nil), c.anyType),\n\t\treturnType: core.OrElse(c.getAwaitedTypeEx(iterationTypes.returnType, errorNode, nil), c.anyType),\n\t\tnextType:   iterationTypes.nextType,\n\t}",
      "\tyieldType := c.getAwaitedTypeEx(iterationTypes.yieldType, errorNode, nil)\n\tAgentOracleObservation(\"iterationAwaitedRecovery:sync:yield\", map[string]any{\"fallbackAny\": yieldType == nil})\n\treturnType := c.getAwaitedTypeEx(iterationTypes.returnType, errorNode, nil)\n\tAgentOracleObservation(\"iterationAwaitedRecovery:sync:return\", map[string]any{\"fallbackAny\": returnType == nil})\n\treturn IterationTypes{\n\t\tyieldType: core.OrElse(yieldType, c.anyType),\n\t\treturnType: core.OrElse(returnType, c.anyType),\n\t\tnextType: iterationTypes.nextType,\n\t}");
    checkerSource = replaceExactly(checkerSource,
      "resolvedMethodParameterType := core.OrElse(resolver.resolveIterationType(methodParameterType, errorNode), c.anyType)",
      "awaitedMethodParameterType := resolver.resolveIterationType(methodParameterType, errorNode)\n\tAgentOracleObservation(\"iterationAwaitedRecovery:\" + methodName + \":parameter\", map[string]any{\"fallbackAny\": awaitedMethodParameterType == nil})\n\tresolvedMethodParameterType := core.OrElse(awaitedMethodParameterType, c.anyType)");
    checkerSource = replaceExactly(checkerSource,
      "resolvedMethodReturnType := core.OrElse(resolver.resolveIterationType(methodReturnType, errorNode), c.anyType)",
      "awaitedMethodReturnType := resolver.resolveIterationType(methodReturnType, errorNode)\n\tAgentOracleObservation(\"iterationAwaitedRecovery:\" + methodName + \":return\", map[string]any{\"fallbackAny\": awaitedMethodReturnType == nil})\n\tresolvedMethodReturnType := core.OrElse(awaitedMethodReturnType, c.anyType)");
    checkerSource = replaceExactly(checkerSource,
      'func (c *Checker) createPromiseType(promisedType *Type) *Type {\n\t// creates a `Promise<T>` type where `T` is the promisedType argument\n\tglobalPromiseType := c.getGlobalPromiseTypeChecked()',
      'func (c *Checker) createPromiseType(promisedType *Type) *Type {\n\t// creates a `Promise<T>` type where `T` is the promisedType argument\n\tglobalPromiseType := c.getGlobalPromiseTypeChecked()\n\tAgentOracleObservation("checkedPromiseFactory", map[string]any{"emptyRecovery": globalPromiseType == c.emptyGenericType})');
    checkerSource = replaceExactly(checkerSource,
      'func (c *Checker) IsDeprecatedDeclaration(declaration *ast.Node) bool {',
      'func (c *Checker) IsDeprecatedDeclaration(declaration *ast.Node) (result bool) {\n\tdefer func() {\n\t\tAgentOracleObservation(fmt.Sprintf("deprecatedConfirmation:%d:%d", declaration.Pos(), declaration.End()), map[string]any{"kind": declaration.Kind, "pos": declaration.Pos(), "end": declaration.End(), "deprecated": result})\n\t}()');
    checkerSource = replaceExactly(checkerSource,
      '\t\tdeprecatedTag := ast.GetJSDocDeprecatedTag(declaration)',
      '\t\tdeprecatedTag := ast.GetJSDocDeprecatedTag(declaration)\n\t\tAgentOracleObservation(fmt.Sprintf("deprecatedSuggestion:%d:%d", declaration.Pos(), declaration.End()), map[string]any{"kind": declaration.Kind, "pos": declaration.Pos(), "end": declaration.End(), "hasTag": deprecatedTag != nil})');
    checkerSource = replaceExactly(checkerSource,
      'func (c *Checker) resolveNewExpression(node *ast.Node, candidatesOutArray *[]*Signature, checkMode CheckMode) *Signature {',
      'func (c *Checker) resolveNewExpression(node *ast.Node, candidatesOutArray *[]*Signature, checkMode CheckMode) (result *Signature) {\n\tdefer func() {\n\t\tAgentOracleObservation(fmt.Sprintf("constructorResolution:%d:%d", node.Pos(), node.End()), map[string]any{"unknownSignature": result == c.unknownSignature, "anySignature": result == c.anySignature, "silentNeverSignature": result == c.silentNeverSignature})\n\t}()');
    checkerSource = replaceExactly(checkerSource,
      '\t\treturn c.getPropertyOfObjectType(c.globalObjectType, name)',
      '\t\tAgentOracleObservation("objectAugmentation:" + name, map[string]any{"emptyRecovery": c.globalObjectType == c.emptyObjectType})\n\t\treturn c.getPropertyOfObjectType(c.globalObjectType, name)');
    checkerSource = replaceExactly(checkerSource,
      'func (c *Checker) getReturnTypeOfSignature(sig *Signature) *Type {',
      'func (c *Checker) getReturnTypeOfSignature(sig *Signature) (result *Type) {\n\tdefer func() {\n\t\tif sig.declaration != nil {\n\t\t\tkey := fmt.Sprintf("signatureReturn:%d:%d:%d", sig.declaration.Kind, sig.declaration.Pos(), sig.declaration.End())\n\t\t\tAgentOracleObservation(key, map[string]any{"kind": sig.declaration.Kind, "pos": sig.declaration.Pos(), "end": sig.declaration.End(), "errorType": result != nil && c.isErrorType(result)})\n\t\t}\n\t}()');
    if (request.inspectNonNullReturns === true) {
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) checkNonNullTypeWithReporter(t *Type, node *ast.Node, reportError func(c *Checker, node *ast.Node, facts TypeFacts)) *Type {',
        'func (c *Checker) checkNonNullTypeWithReporter(t *Type, node *ast.Node, reportError func(c *Checker, node *ast.Node, facts TypeFacts)) (result *Type) {\n\tdefer func() {\n\t\tAgentOracleObservation(fmt.Sprintf("nonNullReturn:%d:%d", node.Pos(), node.End()), map[string]any{"errorType": result == c.errorType, "pos": node.Pos(), "end": node.End()})\n\t}()');
    }
    // Observe the already returned singleton identity, without asking another
    // checker question or demanding a synthetic signature's absent declaration.
    if (request.inspectUnknownSignatureReturns === true) {
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getReturnTypeOfSignature(sig *Signature) (result *Type) {',
        'func (c *Checker) getReturnTypeOfSignature(sig *Signature) (result *Type) {\n\tdefer func() {\n\t\tif sig == c.unknownSignature {\n\t\t\tAgentOracleCounters["unknownSignatureReturns"]++\n\t\t\tAgentOracleObservation("unknownSignatureReturn", map[string]any{"errorType": result == c.errorType})\n\t\t}\n\t}()');
    }
    checkerSource = replaceExactly(checkerSource,
      'func (c *Checker) isBlockScopedNameDeclaredBeforeUse(declaration *ast.Node, usage *ast.Node) bool {',
      'func (c *Checker) isBlockScopedNameDeclaredBeforeUse(declaration *ast.Node, usage *ast.Node) (result bool) {\n\tdefer func() {\n\t\tkey := fmt.Sprintf("beforeUse:%d:%d", declaration.Pos(), usage.Pos())\n\t\tAgentOracleObservation(key, map[string]any{"result": result, "declarationKind": declaration.Kind, "declarationPos": declaration.Pos(), "declarationEnd": declaration.End(), "usagePos": usage.Pos(), "usageEnd": usage.End()})\n\t}()');
    checkerSource = replaceExactly(checkerSource,
      '\t\t\tflowType := c.getFlowTypeOfReferenceEx(reference, propType, c.getOptionalType(propType, false), nil, nil)',
      '\t\t\tinitialType := c.getOptionalType(propType, false)\n\t\t\tflowType := c.getFlowTypeOfReferenceEx(reference, propType, initialType, nil, nil)\n\t\t\tAgentOracleObservation(fmt.Sprintf("staticBlockFlow:%d:%d", propName.Pos(), staticBlock.Pos()), map[string]any{"propertyPos": propName.Pos(), "propertyEnd": propName.End(), "blockPos": staticBlock.Pos(), "blockEnd": staticBlock.End(), "sameInitial": flowType == initialType, "sameDeclared": flowType == propType, "errorType": c.isErrorType(flowType), "initialFlags": uint32(initialType.flags), "resultFlags": uint32(flowType.flags)})');
    checkerSource = replaceExactly(checkerSource,
      'func (c *Checker) isReadonlySymbol(symbol *ast.Symbol) bool {',
      'func (c *Checker) isReadonlySymbol(symbol *ast.Symbol) (result bool) {\n\tdefer func() {\n\t\tif symbol.Flags&ast.SymbolFlagsPrototype != 0 { AgentOracleObservation("prototypeReadonly", map[string]any{"readonly": result}) }\n\t}()');
    checkerSource = replaceExactly(checkerSource,
      'func (b *keyBuilder) writeSymbol(s *ast.Symbol) {\n\thashWrite64(&b.h, ast.GetSymbolId(s))\n}',
      'func (b *keyBuilder) writeSymbol(s *ast.Symbol) {\n\tid := ast.GetSymbolId(s)\n\tif s.Name == "key" { AgentOracleObservation("flowKeySymbolID", map[string]any{"id": uint64(id), "name": s.Name}) }\n\thashWrite64(&b.h, id)\n}');
    checkerSource = replaceExactly(checkerSource,
      'func (c *Checker) getDefaultFromTypeParameter(t *Type) *Type {',
      'func (c *Checker) getDefaultFromTypeParameter(t *Type) (result *Type) {\n\tdefer func() {\n\t\tif result != nil && c.isErrorType(result) {\n\t\t\tname := ""\n\t\t\tif t.symbol != nil { name = t.symbol.Name }\n\t\t\tAgentOracleObservation("typeParameterDefaultError", map[string]any{"parameter": name, "hasSymbol": t.symbol != nil})\n\t\t}\n\t}()');
    checkerSource = replaceExactly(checkerSource,
      '\t\t\t\tif s == nil {\n\t\t\t\t\ts = c.errorType\n\t\t\t\t}\n\t\t\t\tn.add(s, TupleElementInfo{flags: ElementFlagsRest, labeledDeclaration: info.labeledDeclaration})',
      '\t\t\t\tif s == nil {\n\t\t\t\t\ts = c.errorType\n\t\t\t\t\tAgentOracleObservation("variadicTupleElementErrorType", map[string]any{"elementIndex": i, "inputNumber": t == c.numberType, "errorType": s == c.errorType})\n\t\t\t\t}\n\t\t\t\tn.add(s, TupleElementInfo{flags: ElementFlagsRest, labeledDeclaration: info.labeledDeclaration})');
    checkerSource = replaceExactly(checkerSource,
      'func (c *Checker) instantiateTypeWorker(t *Type, m *TypeMapper, alias *TypeAlias) *Type {\n\tflags := t.flags\n\tswitch {\n\tcase flags&TypeFlagsTypeParameter != 0:\n\t\treturn m.Map(t)',
      'func (c *Checker) instantiateTypeWorker(t *Type, m *TypeMapper, alias *TypeAlias) *Type {\n\tflags := t.flags\n\tswitch {\n\tcase flags&TypeFlagsTypeParameter != 0:\n\t\tresult := m.Map(t)\n\t\tif c.isErrorType(result) {\n\t\t\tname := ""\n\t\t\tif t.symbol != nil { name = t.symbol.Name }\n\t\t\tAgentOracleObservation("typeParameterMappedError", map[string]any{"parameter": name, "hasSymbol": t.symbol != nil})\n\t\t}\n\t\treturn result');
    checkerSource = replaceExactly(checkerSource,
      'func (c *Checker) resolveTypeReferenceName(typeReference *ast.Node, meaning ast.SymbolFlags, ignoreErrors bool) *ast.Symbol {',
      'func (c *Checker) resolveTypeReferenceName(typeReference *ast.Node, meaning ast.SymbolFlags, ignoreErrors bool) (result *ast.Symbol) {\n\tdefer func() {\n\t\tif meaning == ast.SymbolFlagsAlias {\n\t\t\talias := result != nil && result != c.unknownSymbol && result.Flags&ast.SymbolFlagsAlias != 0\n\t\t\tname := ""\n\t\t\tdeclarations := []map[string]any{}\n\t\t\tif result != nil { name = result.Name; for _, declaration := range result.Declarations { declarations = append(declarations, map[string]any{"kind": declaration.Kind, "pos": declaration.Pos(), "end": declaration.End()}) } }\n\t\t\tAgentOracleObservation(fmt.Sprintf("typeReferenceAlias:%d:%d", typeReference.Pos(), typeReference.End()), map[string]any{"pos": typeReference.Pos(), "end": typeReference.End(), "alias": alias, "name": name, "declarations": declarations})\n\t\t}\n\t}()');
    checkerSource = replaceExactly(checkerSource,
      'func (c *Checker) getTypeFromTypeReference(node *ast.Node) *Type {\n\tlinks := c.typeNodeLinks.Get(node)\n\tif links.resolvedType == nil {',
      'func (c *Checker) getTypeFromTypeReference(node *ast.Node) *Type {\n\tlinks := c.typeNodeLinks.Get(node)\n\tif links.resolvedType != nil {\n\t\tAgentOracleCounters["typeReferenceCacheHit"]++\n\t\tAgentOracleObservation("typeReferenceCacheHit", map[string]any{"kind": node.Kind, "pos": node.Pos(), "end": node.End()})\n\t} else {\n\t\tAgentOracleCounters["typeReferenceCacheMiss"]++\n\t}\n\tif links.resolvedType == nil {');
    checkerSource = replaceExactly(checkerSource,
      'func (c *Checker) getNonMissingTypeOfSymbol(symbol *ast.Symbol) *Type {\n\treturn c.removeMissingType(c.getTypeOfSymbol(symbol), symbol.Flags&ast.SymbolFlagsOptional != 0)\n}',
      'func (c *Checker) getNonMissingTypeOfSymbol(symbol *ast.Symbol) *Type {\n\tresult := c.removeMissingType(c.getTypeOfSymbol(symbol), symbol.Flags&ast.SymbolFlagsOptional != 0)\n\tif c.isErrorType(result) {\n\t\tAgentOracleObservation("nonMissingSymbolErrorType", map[string]any{"name": symbol.Name, "instantiated": symbol.CheckFlags&ast.CheckFlagsInstantiated != 0})\n\t}\n\treturn result\n}');
    checkerSource = replaceExactly(checkerSource, '\tif c.instantiationDepth == 100 || c.instantiationCount >= 5_000_000 {',
      '\tAgentOracleCounter("instantiationDepth", int(c.instantiationDepth))\n\tAgentOracleCounter("instantiationCount", int(c.instantiationCount))\n\tif c.instantiationDepth == 100 || c.instantiationCount >= 5_000_000 {\n\t\tAgentOracleFailure("instantiation_limit")');
    checkerSource = replaceExactly(checkerSource, '\t\tif tailCount == 1000 {',
      '\t\tAgentOracleCounter("conditionalTail", tailCount)\n\t\tif tailCount == 1000 {\n\t\t\tAgentOracleFailure("conditional_tail_limit")');
    checkerSource = replaceExactly(checkerSource, '\tif size >= 100_000 {',
      '\tAgentOracleCounter("crossProductUnion", size)\n\tif size >= 100_000 {\n\t\tAgentOracleFailure("union_complexity_limit")');
    checkerSource = replaceExactly(checkerSource, '\t\t\t\t\t\testimatedCount := (count / (length - i)) * length',
      '\t\t\t\t\t\testimatedCount := (count / (length - i)) * length\n\t\t\t\t\t\tAgentOracleObservation("subtypeEstimate", map[string]any{"count": count, "length": length, "processed": length-i, "estimate": estimatedCount})');
    checkerSource = replaceExactly(checkerSource, '\t\t\t\t\t\tif estimatedCount > 1000000 {',
      '\t\t\t\t\t\tif estimatedCount > 1000000 {\n\t\t\t\t\t\t\tAgentOracleFailure("subtype_complexity_limit")');
    checkerSource = replaceExactly(checkerSource,
      '\t\tif links.resolvedSignature != c.resolvingSignature {\n\t\t\tresult = links.resolvedSignature',
      '\t\tif links.resolvedSignature != c.resolvingSignature {\n\t\t\tAgentOracleObservation(fmt.Sprintf("resolvedSignatureReentry:%d:%d", node.Pos(), node.End()), map[string]any{"kind": node.Kind, "pos": node.Pos(), "end": node.End(), "sameResult": result == links.resolvedSignature, "unknownResult": result == c.unknownSignature, "diagnosticCodes": c.AgentOracleCurrentDiagnostics()})\n\t\t\tresult = links.resolvedSignature');
    if (request.inspectTemplateResources === true) {
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getTemplateLiteralType(texts []string, types []*Type) *Type {',
        'func (c *Checker) getTemplateLiteralType(texts []string, types []*Type) *Type {\n\tAgentOracleCounters["templateLiteralCalls"]++');
      checkerSource = replaceExactly(checkerSource,
        '\t\treturn c.getStringLiteralType(stringutil.CombineSurrogatePairs(sb.String()))',
        '\t\traw := sb.String()\n\t\tnormalized := stringutil.CombineSurrogatePairs(raw)\n\t\tAgentOracleCounters["templateNormalizationInputBytes"] += len(raw)\n\t\tAgentOracleCounters["templateNormalizationOutputBytes"] += len(normalized)\n\t\treturn c.getStringLiteralType(normalized)');
    }
    if (request.inspectStringMappings === true) {
      // Observe actual calls only; do not evaluate another type or string map.
      checkerSource = replaceExactly(checkerSource,
        'func applyStringMapping(symbol *ast.Symbol, str string) string {',
        'func applyStringMapping(symbol *ast.Symbol, str string) string {\n\tAgentOracleCounters["stringMapping:" + symbol.Name]++');
    }
    if (request.inspectRelationKeys === true) {
      checkerSource = replaceExactly(checkerSource,
        'func (b *keyBuilder) writeGenericTypeReferences(source *Type, target *Type, ignoreConstraints bool) bool {',
        'func (b *keyBuilder) writeGenericTypeReferences(source *Type, target *Type, ignoreConstraints bool) bool {\n\tAgentOracleCounters["genericKeyBuilds"]++');
      checkerSource = replaceExactly(checkerSource,
        '\t\t\t\t\tindex := slices.Index(typeParameters, t)',
        '\t\t\t\t\tindex := slices.Index(typeParameters, t)\n\t\t\t\t\tif index < 0 { AgentOracleCounters["genericKeyParameterComparisons"] += len(typeParameters) } else { AgentOracleCounters["genericKeyParameterComparisons"] += index + 1 }');
      checkerSource = replaceExactly(checkerSource,
        '\t\tfor _, t := range ref.AsTypeReference().resolvedTypeArguments {',
        '\t\tfor _, t := range ref.AsTypeReference().resolvedTypeArguments {\n\t\t\tAgentOracleCounters["genericKeyArguments"]++');
    }
    if (request.inspectLiteralCaches === true) {
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getStringLiteralType(value string) *Type {\n\tt := c.stringLiteralTypes[value]',
        'func (c *Checker) getStringLiteralType(value string) *Type {\n\tAgentOracleCounters["stringLiteralLookups"]++\n\tAgentOracleCounters["stringLiteralKeyBytes"] += len(value)\n\tt := c.stringLiteralTypes[value]\n\tif t == nil { AgentOracleCounters["stringLiteralMisses"]++ }');
    }
    if (request.inspectUnresolvedNameCaches === true) {
      // Existing intern lookups expose exact decoded paths without new demands.
      checkerSource = replaceExactly(checkerSource,
        '\t\tresult := c.unresolvedSymbols[path]',
        '\t\tAgentOracleCounters["unresolvedNameKeyBytes"] += len(path)\n\t\tif len(path) > AgentOracleCounters["unresolvedNameMaxBytes"] { AgentOracleCounters["unresolvedNameMaxBytes"] = len(path) }\n\t\tresult := c.unresolvedSymbols[path]\n\t\tif result == nil { AgentOracleCounters["unresolvedNameMissBytes"] += len(path) } else { AgentOracleCounters["unresolvedNameHitBytes"] += len(path) }');
    }
    if (request.inspectSymbolTypeCacheResources === true) {
      // Existing symbol links only; do not acquire another symbol/type/ID.
      checkerSource = replaceExactly(checkerSource,
        "func (c *Checker) getTypeOfVariableOrParameterOrProperty(symbol *ast.Symbol) *Type {\n\tlinks := c.valueSymbolLinks.Get(symbol)",
        "func (c *Checker) getTypeOfVariableOrParameterOrProperty(symbol *ast.Symbol) *Type {\n\tlinks := c.valueSymbolLinks.Get(symbol)\n\tdefer func() {\n\t\tif strings.HasPrefix(symbol.Name, \"structureSlot\") && links.resolvedType != nil { AgentOracleObservation(\"symbolTypeSlot:value:\"+symbol.Name, map[string]any{\"name\": symbol.Name, \"installed\": true}) }\n\t}()");
      checkerSource = replaceExactly(checkerSource,
        "func (c *Checker) getDeclaredTypeOfTypeAlias(symbol *ast.Symbol) *Type {\n\tlinks := c.typeAliasLinks.Get(symbol)",
        "func (c *Checker) getDeclaredTypeOfTypeAlias(symbol *ast.Symbol) *Type {\n\tlinks := c.typeAliasLinks.Get(symbol)\n\tdefer func() {\n\t\tif strings.HasPrefix(symbol.Name, \"StructureSlot\") && links.declaredType != nil { AgentOracleObservation(\"symbolTypeSlot:declared:\"+symbol.Name, map[string]any{\"name\": symbol.Name, \"installed\": true}) }\n\t}()");
    }
    if (request.inspectStructureCacheResources === true) {
      // Read installed native slots only; never obtain another type or ID.
      checkerSource = replaceExactly(checkerSource,
        "func (c *Checker) resolveStructuredTypeMembers(t *Type) *StructuredType {",
        "func (c *Checker) resolveStructuredTypeMembers(t *Type) *StructuredType {\n\tdefer func() {\n\t\tif t.objectFlags&ObjectFlagsMembersResolved != 0 { AgentOracleObservation(\"structureSlot:members:\"+strconv.Itoa(int(t.id)), map[string]any{\"installed\": true, \"tableNil\": t.AsStructuredType().members == nil}) }\n\t}()");
      checkerSource = replaceExactly(checkerSource,
        "func (c *Checker) getApparentTypeOfIntersectionType(t *Type, thisArgument *Type) *Type {",
        "func (c *Checker) getApparentTypeOfIntersectionType(t *Type, thisArgument *Type) *Type {\n\tdefer func() {\n\t\tvar installed bool\n\t\tif t == thisArgument { installed = t.AsIntersectionType().resolvedApparentType != nil } else { installed = c.cachedTypes[CachedTypeKey{kind: CachedTypeKindApparentType, typeId: thisArgument.id}] != nil }\n\t\tif installed { AgentOracleObservation(\"structureSlot:apparent:\"+strconv.Itoa(int(thisArgument.id)), map[string]any{\"installed\": true}) }\n\t}()");
      checkerSource = replaceExactly(checkerSource,
        "func (c *Checker) getReducedType(t *Type) *Type {",
        "func (c *Checker) getReducedType(t *Type) *Type {\n\tdefer func() {\n\t\tif t.flags&TypeFlagsIntersection != 0 && t.objectFlags&ObjectFlagsIsNeverIntersectionComputed != 0 { AgentOracleObservation(\"structureSlot:reduced:\"+strconv.Itoa(int(t.id)), map[string]any{\"installed\": true}) }\n\t}()");
      checkerSource = replaceExactly(checkerSource,
        "func (c *Checker) getGenericObjectFlags(t *Type) ObjectFlags {",
        "func (c *Checker) getGenericObjectFlags(t *Type) ObjectFlags {\n\tdefer func() {\n\t\tif t.flags&(TypeFlagsUnionOrIntersection|TypeFlagsSubstitution) != 0 && t.objectFlags&ObjectFlagsIsGenericTypeComputed != 0 { AgentOracleObservation(\"structureSlot:generic:\"+strconv.Itoa(int(t.id)), map[string]any{\"installed\": true}) }\n\t}()");
      checkerSource = replaceExactly(checkerSource,
        "func (c *Checker) getRegularTypeOfObjectLiteral(t *Type) *Type {",
        "func (c *Checker) getRegularTypeOfObjectLiteral(t *Type) *Type {\n\tdefer func() {\n\t\tif isObjectLiteralType(t) && t.objectFlags&ObjectFlagsFreshLiteral != 0 && c.cachedTypes[CachedTypeKey{kind: CachedTypeKindRegularObjectLiteral, typeId: t.id}] != nil { AgentOracleObservation(\"structureSlot:regular:\"+strconv.Itoa(int(t.id)), map[string]any{\"installed\": true}) }\n\t}()");
    }
    if (request.inspectSignatureAssociations === true) {
      // Observe installed factory/declaration and return properties only;
      // do not obtain a symbol/signature ID or another semantic result.
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getSignatureFromDeclaration(declaration *ast.Node) *Signature {\n\tlinks := c.signatureLinks.Get(declaration)',
        'func (c *Checker) getSignatureFromDeclaration(declaration *ast.Node) *Signature {\n\tlinks := c.signatureLinks.Get(declaration)\n\tdefer func() {\n\t\tif declaration.Name() != nil && strings.HasPrefix(declaration.Name().Text(), "CachedCall") && links.resolvedSignature != nil { AgentOracleObservation("signatureAssociation:factory:"+declaration.Name().Text(), map[string]any{"name": declaration.Name().Text(), "hasDeclaration": links.resolvedSignature.declaration == declaration}) }\n\t}()');
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getReturnTypeOfSignature(sig *Signature) (result *Type) {',
        'func (c *Checker) getReturnTypeOfSignature(sig *Signature) (result *Type) {\n\tdefer func() {\n\t\tif sig.declaration != nil && sig.declaration.Name() != nil && strings.HasPrefix(sig.declaration.Name().Text(), "CachedCall") && sig.resolvedReturnType != nil { AgentOracleObservation("signatureAssociation:return:"+sig.declaration.Name().Text(), map[string]any{"name": sig.declaration.Name().Text(), "number": sig.resolvedReturnType == c.numberType}) }\n\t}()');
    }
    if (request.inspectResolvedCallSlots === true) {
      // Read the actual signatureLinks property at return; no new signature,
      // symbol/signature ID or checker question is acquired.
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getResolvedSignature(node *ast.Node, candidatesOutArray *[]*Signature, checkMode CheckMode) *Signature {\n\tlinks := c.signatureLinks.Get(node)',
        'func (c *Checker) getResolvedSignature(node *ast.Node, candidatesOutArray *[]*Signature, checkMode CheckMode) *Signature {\n\tlinks := c.signatureLinks.Get(node)\n\tdefer func() {\n\t\tif links.resolvedSignature != nil && links.resolvedSignature != c.resolvingSignature { AgentOracleObservation(fmt.Sprintf("callSlot:resolved:%d:%d", node.Pos(), node.End()), map[string]any{"pos": node.Pos(), "end": node.End(), "unknown": links.resolvedSignature == c.unknownSignature}) }\n\t}()');
    }
    if (request.inspectUniqueCacheSlots === true) {
      // Both observations read fields already installed by the native call.
      // They acquire no second symbol, type, node link or checker demand.
      checkerSource = replaceExactly(checkerSource,
        '\t\t\tc.uniqueESSymbolTypes[symbol] = uniqueType',
        '\t\t\tc.uniqueESSymbolTypes[symbol] = uniqueType\n\t\t\tif strings.HasPrefix(symbol.Name, "UniqueSlot") { AgentOracleObservation(fmt.Sprintf("uniqueSlot:symbol:%s:%d", symbol.Name, uniqueType.id), map[string]any{"name": symbol.Name, "typeId": uniqueType.id}) }');
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getTypeFromTypeOperatorNode(node *ast.Node) *Type {\n\tlinks := c.typeNodeLinks.Get(node)',
        'func (c *Checker) getTypeFromTypeOperatorNode(node *ast.Node) *Type {\n\tlinks := c.typeNodeLinks.Get(node)\n\tdefer func() {\n\t\tif node.AsTypeOperatorNode().Operator == ast.KindUniqueKeyword && links.resolvedType != nil { AgentOracleObservation(fmt.Sprintf("uniqueSlot:node:%d:%d", node.Pos(), node.End()), map[string]any{"pos": node.Pos(), "end": node.End(), "typeId": links.resolvedType.id}) }\n\t}()');
    }
    if (request.inspectConstraintCacheSlots === true) {
      // Observe existing identities and installed fields. No extra constraint,
      // symbol ID, base computation or checker demand is acquired.
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) newTypeParameter(symbol *ast.Symbol) *Type {',
        'func (c *Checker) newTypeParameter(symbol *ast.Symbol) *Type {\n\tif symbol != nil && (strings.HasPrefix(symbol.Name, "ConstraintParam") || strings.HasPrefix(symbol.Name, "ConstraintInfer")) { AgentOracleObservation("constraintSlot:parameter:"+symbol.Name, map[string]any{"name": symbol.Name}) }');
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getInferTypeParameters(node *ast.Node) []*Type {',
        'func (c *Checker) getInferTypeParameters(node *ast.Node) []*Type {\n\tfor _, symbol := range node.Locals() {\n\t\tif symbol.Flags&ast.SymbolFlagsTypeParameter != 0 && strings.HasPrefix(symbol.Name, "ConstraintInfer") { AgentOracleObservation("constraintSlot:inferred:"+symbol.Name, map[string]any{"name": symbol.Name}) }\n\t}');
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getConstraintFromTypeParameter(t *Type) *Type {',
        'func (c *Checker) getConstraintFromTypeParameter(t *Type) *Type {\n\tdefer func() {\n\t\tif t.flags&TypeFlagsTypeParameter != 0 && t.symbol != nil && strings.HasPrefix(t.symbol.Name, "ConstraintParam") && t.AsTypeParameter().constraint != nil {\n\t\t\tAgentOracleObservation("constraintSlot:raw:"+t.symbol.Name, map[string]any{"name": t.symbol.Name, "absent": t.AsTypeParameter().constraint == c.noConstraintType})\n\t\t}\n\t}()');
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getResolvedBaseConstraint(t *Type, stack []RecursionId) *Type {',
        'func (c *Checker) getResolvedBaseConstraint(t *Type, stack []RecursionId) *Type {\n\tdefer func() {\n\t\tif t.symbol != nil && strings.HasPrefix(t.symbol.Name, "ConstraintParam") {\n\t\t\tif constrained := t.AsConstrainedType(); constrained != nil && constrained.resolvedBaseConstraint != nil { AgentOracleObservation("constraintSlot:base:"+t.symbol.Name, map[string]any{"name": t.symbol.Name, "absent": constrained.resolvedBaseConstraint == c.noConstraintType}) }\n\t\t}\n\t}()');
    }
    if (request.inspectParameterNamePayloads === true) {
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) newTypeParameter(symbol *ast.Symbol) *Type {',
        'func (c *Checker) newTypeParameter(symbol *ast.Symbol) *Type {\n\tif symbol != nil && strings.Contains(symbol.Name, "NamesParam") { AgentOracleCounters["parameterNames:owned"]++ }');
    }
    if (request.inspectAwaitedConstraintScratch === true) {
      checkerSource = replaceExactly(checkerSource,
        '\t\t\treturn baseConstraint.flags&TypeFlagsAnyOrUnknown != 0 || c.isEmptyObjectType(baseConstraint) || someType(baseConstraint, c.isThenableType)',
        '\t\t\treturn baseConstraint.flags&TypeFlagsAnyOrUnknown != 0 || c.isEmptyObjectType(baseConstraint) || func() bool {\n\t\t\t\tvisits := 0\n\t\t\t\tresult := someType(baseConstraint, func(alternative *Type) bool { visits++; return c.isThenableType(alternative) })\n\t\t\t\tAgentOracleCounters["awaitedConstraint:visits"] += visits\n\t\t\t\tAgentOracleCounters["awaitedConstraint:maximum"] = max(AgentOracleCounters["awaitedConstraint:maximum"], visits)\n\t\t\t\treturn result\n\t\t\t}()');
    }
    if (request.inspectThenableScratch === true) {
      checkerSource = replaceExactly(checkerSource,
        '\tfor _, thenSignature := range thenSignatures {',
        '\tfor _, thenSignature := range thenSignatures {\n\t\tAgentOracleCounters["thenScratch:filterVisits"]++');
      checkerSource = replaceExactly(checkerSource,
        '\tonfulfilledParameterType := c.getTypeWithFacts(c.getUnionType(core.Map(candidates, c.getTypeOfFirstParameterOfSignature)), TypeFactsNEUndefinedOrNull)',
        '\tAgentOracleCounters["thenScratch:parameterVisits"] += len(candidates)\n\tonfulfilledParameterType := c.getTypeWithFacts(c.getUnionType(core.Map(candidates, c.getTypeOfFirstParameterOfSignature)), TypeFactsNEUndefinedOrNull)');
      checkerSource = replaceExactly(checkerSource,
        '\tresult := c.getUnionTypeEx(core.Map(onfulfilledParameterSignatures, c.getTypeOfFirstParameterOfSignature), UnionReductionSubtype, nil, nil)',
        '\tAgentOracleCounters["thenScratch:callbackVisits"] += len(onfulfilledParameterSignatures)\n\tresult := c.getUnionTypeEx(core.Map(onfulfilledParameterSignatures, c.getTypeOfFirstParameterOfSignature), UnionReductionSubtype, nil, nil)');
    }
    if (request.inspectSignaturePayloads === true) {
      // Observe the existing constructor arguments and inventory result only;
      // do not acquire a new signature, symbol ID or parameter type.
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) newSignature(flags SignatureFlags, declaration *ast.Node, typeParameters []*Type, thisParameter *ast.Symbol, parameters []*ast.Symbol, resolvedReturnType *Type, resolvedTypePredicate *TypePredicate, minArgumentCount int) *Signature {',
        'func (c *Checker) newSignature(flags SignatureFlags, declaration *ast.Node, typeParameters []*Type, thisParameter *ast.Symbol, parameters []*ast.Symbol, resolvedReturnType *Type, resolvedTypePredicate *TypePredicate, minArgumentCount int) *Signature {\n\tAgentOracleCounters["signaturePayload:parameters"] += len(parameters)\n\tAgentOracleCounters["signaturePayload:maximumParameters"] = max(AgentOracleCounters["signaturePayload:maximumParameters"], len(parameters))\n\tif declaration != nil { AgentOracleCounters["signaturePayload:declared"]++ }');
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getSignaturesOfType(t *Type, kind SignatureKind) []*Signature {\n\treturn c.getSignaturesOfStructuredType(c.getReducedApparentType(t), kind)\n}',
        'func (c *Checker) getSignaturesOfType(t *Type, kind SignatureKind) []*Signature {\n\tresult := c.getSignaturesOfStructuredType(c.getReducedApparentType(t), kind)\n\tAgentOracleCounters["signaturePayload:inventoryMaximum"] = max(AgentOracleCounters["signaturePayload:inventoryMaximum"], len(result))\n\treturn result\n}');
    }
    if (request.inspectPromiseCacheSlots === true) {
      // Observe only already-published cachedTypes entries on existing calls.
      // Reading the existing type IDs does not acquire a type or symbol ID.
      for (const [signature, kind, label] of [
        ['func (c *Checker) getPromisedTypeOfPromiseEx(t *Type, errorNode *ast.Node, thisTypeForErrorOut **Type) *Type {',
          'CachedTypeKindPromisedTypeOfPromise', 'promised'],
        ['func (c *Checker) getAwaitedTypeNoAliasEx(t *Type, errorNode *ast.Node, diagnosticMessage *diagnostics.Message, args ...any) *Type {',
          'CachedTypeKindAwaitedType', 'awaited'],
      ]) {
        checkerSource = replaceExactly(checkerSource, signature,
          signature + `\n\tdefer func() {\n\t\tif cached := c.cachedTypes[CachedTypeKey{kind: ${kind}, typeId: t.id}]; cached != nil {\n\t\t\tAgentOracleObservation(fmt.Sprintf("promiseCache:${label}:%d", t.id), map[string]any{"typeId": t.id, "resultId": cached.id})\n\t\t}\n\t}()`);
      }
    }
    if (request.inspectPromiseTraversal === true) {
      for (const [signature, label] of [
        ['func (c *Checker) getPromisedTypeOfPromiseEx(t *Type, errorNode *ast.Node, thisTypeForErrorOut **Type) *Type {', 'promised'],
        ['func (c *Checker) getAwaitedTypeNoAliasEx(t *Type, errorNode *ast.Node, diagnosticMessage *diagnostics.Message, args ...any) *Type {', 'awaited'],
      ]) {
        checkerSource = replaceExactly(checkerSource, signature,
          signature + `\n\tAgentOracleCounters["promiseTraversal:${label}Calls"]++\n\tAgentOracleCounters["promiseTraversal:${label}Depth"]++\n\tAgentOracleCounters["promiseTraversal:${label}PeakDepth"] = max(AgentOracleCounters["promiseTraversal:${label}PeakDepth"], AgentOracleCounters["promiseTraversal:${label}Depth"])\n\tdefer func() { AgentOracleCounters["promiseTraversal:${label}Depth"]-- }()`);
      }
      // slices.Contains over the existing stack: preserve its pointer equality
      // and first-match short circuit without acquiring another native value.
      checkerSource = replaceExactly(checkerSource,
        'slices.Contains(c.awaitedTypeStack, t)', 'AgentOracleAwaitedContains(c.awaitedTypeStack, t)');
      checkerSource = replaceExactly(checkerSource,
        'slices.Contains(c.awaitedTypeStack, promisedType)', 'AgentOracleAwaitedContains(c.awaitedTypeStack, promisedType)');
      checkerSource += '\nfunc AgentOracleAwaitedContains(stack []*Type, target *Type) bool {\n\tfor _, candidate := range stack {\n\t\tAgentOracleCounters["promiseTraversal:stackComparisons"]++\n\t\tif candidate == target { return true }\n\t}\n\treturn false\n}\n';
    }
    if (request.inspectAliasParameterSlots === true) {
      // Observe existing alias declarations only. This does not acquire a type,
      // symbol ID, parameter default or a second checker demand.
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getLocalTypeParametersOfClassOrInterfaceOrTypeAlias(symbol *ast.Symbol) []*Type {',
        'func (c *Checker) getLocalTypeParametersOfClassOrInterfaceOrTypeAlias(symbol *ast.Symbol) []*Type {\n\tif symbol.Flags&ast.SymbolFlagsTypeAlias != 0 {\n\t\tfor _, node := range symbol.Declarations {\n\t\t\tif isTypeAlias(node) {\n\t\t\t\tAgentOracleObservation(fmt.Sprintf("aliasParameterSlot:%s:%d", symbol.Name, node.Pos()), map[string]any{"name": symbol.Name, "pos": node.Pos(), "parameterCount": len(node.TypeParameters())})\n\t\t\t}\n\t\t}\n\t}');
    }
    if (request.inspectSymbolNames === true) {
      // Observe the existing alias header; no name lookup or type acquisition.
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getDeclaredTypeOfTypeAlias(symbol *ast.Symbol) *Type {',
        'func (c *Checker) getDeclaredTypeOfTypeAlias(symbol *ast.Symbol) *Type {\n\tAgentOracleCounters["symbolAliasNameBytes"] += len(symbol.Name)\n\tAgentOracleCounters["symbolAliasNameMaxBytes"] = max(AgentOracleCounters["symbolAliasNameMaxBytes"], len(symbol.Name))');
    }
    if (request.inspectSymbolHeaders === true) {
      // Count the existing factory only; do not acquire another symbol/type.
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) newSymbol(flags ast.SymbolFlags, name string) *ast.Symbol {',
        'func (c *Checker) newSymbol(flags ast.SymbolFlags, name string) *ast.Symbol {\n\tAgentOracleCounters["symbolHeaderCreates"]++\n\tAgentOracleCounters["symbolHeaderNameBytes"] += len(name)');
    }
    if (request.inspectContainment === true) {
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) couldContainTypeVariablesWorker(t *Type) bool {',
        'func (c *Checker) couldContainTypeVariablesWorker(t *Type) bool {\n\tAgentOracleCounters["containmentCalls"]++\n\tAgentOracleCounters["containmentDepth"]++\n\tAgentOracleCounters["containmentPeakDepth"] = max(AgentOracleCounters["containmentPeakDepth"], AgentOracleCounters["containmentDepth"])\n\tdefer func() { AgentOracleCounters["containmentDepth"]-- }()');
    }
    if (request.inspectObjectNameTables === true) {
      checkerSource = replaceExactly(checkerSource,
        'func (w *WideningContext) getChildContext(propertyName string) *WideningContext {',
        'func (w *WideningContext) getChildContext(propertyName string) *WideningContext {\n\tAgentOracleCounters["wideningChildBytes"] += len(propertyName)\n\tif w.childContexts[propertyName] != nil { AgentOracleCounters["wideningChildHitBytes"] += len(propertyName) }');
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getUndefinedProperty(prop *ast.Symbol) *ast.Symbol {',
        'func (c *Checker) getUndefinedProperty(prop *ast.Symbol) *ast.Symbol {\n\tAgentOracleCounters["wideningUndefinedBytes"] += len(prop.Name)\n\tif c.undefinedProperties[prop.Name] != nil { AgentOracleCounters["wideningUndefinedHitBytes"] += len(prop.Name) }');
      checkerSource = replaceExactly(checkerSource,
        '\t\t\t\t\tnames.Set(prop.Name, prop)',
        '\t\t\t\t\tAgentOracleCounters["wideningInventoryBytes"] += len(prop.Name)\n\t\t\t\t\tnames.Set(prop.Name, prop)');
      checkerSource = replaceExactly(checkerSource,
        '\t\t\tif left := props[right.Name]; left != nil {',
        '\t\t\tAgentOracleCounters["spreadNameBytes"] += len(right.Name)\n\t\t\tif left := props[right.Name]; left != nil {\n\t\t\t\tAgentOracleCounters["spreadNameHitBytes"] += len(right.Name)');
      let grammarSource = fs.readFileSync(path.join(sourceDirectory, 'internal/checker/grammarchecks.go'), 'utf8');
      grammarSource = replaceExactly(grammarSource,
        '\t\t\texistingKind := seen[effectiveName]',
        '\t\t\tAgentOracleCounters["grammarNameBytes"] += len(effectiveName)\n\t\t\texistingKind := seen[effectiveName]\n\t\t\tif existingKind != 0 { AgentOracleCounters["grammarNameHitBytes"] += len(effectiveName) }');
      const overlaidGrammar = path.join(temporary, 'grammarchecks.go');
      fs.writeFileSync(overlaidGrammar, grammarSource);
      attach(overlaidGrammar, 'internal/checker/grammarchecks.go');
    }
    if (request.inspectCompositePropertyCaches === true) {
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) getUnionOrIntersectionProperty(t *Type, name string, skipObjectFunctionPropertyAugment bool) *ast.Symbol {',
        'func (c *Checker) getUnionOrIntersectionProperty(t *Type, name string, skipObjectFunctionPropertyAugment bool) *ast.Symbol {\n\tAgentOracleCounters["compositeKeyBytes"] += len(name)\n\tAgentOracleCounters["compositeMaxKeyBytes"] = max(AgentOracleCounters["compositeMaxKeyBytes"], len(name))\n\tif skipObjectFunctionPropertyAugment { AgentOracleCounters["compositeUnaugmentedLookups"]++ } else { AgentOracleCounters["compositeAugmentedLookups"]++ }');
      checkerSource = replaceExactly(checkerSource,
        '\tif prop := cache[name]; prop != nil {\n\t\treturn prop\n\t}\n\tprop := c.createUnionOrIntersectionProperty',
        '\tif prop := cache[name]; prop != nil {\n\t\tAgentOracleCounters["compositeHitBytes"] += len(name)\n\t\treturn prop\n\t}\n\tAgentOracleCounters["compositeMissBytes"] += len(name)\n\tprop := c.createUnionOrIntersectionProperty');
    }
    if (request.inspectUnionReductionCaches === true) {
      checkerSource = replaceExactly(checkerSource,
        '\t\tkey := UnionOfUnionKey{id1: id1, id2: id2, r: unionReduction, a: getAliasKey(alias)}\n\t\tt := c.unionOfUnionTypes[key]',
        '\t\tkey := UnionOfUnionKey{id1: id1, id2: id2, r: unionReduction, a: getAliasKey(alias)}\n\t\tAgentOracleCounters["unionPairLookups"]++\n\t\tif alias != nil { AgentOracleCounters["unionPairAliasArguments"] += len(alias.typeArguments) }\n\t\tt := c.unionOfUnionTypes[key]');
      checkerSource = replaceExactly(checkerSource,
        '\tkey := getTypeListKey(types)\n\tif cached := c.subtypeReductionCache[key]; cached != nil {\n\t\treturn cached\n\t}',
        '\tkey := getTypeListKey(types)\n\tAgentOracleCounters["subtypeKeyArguments"] += len(types)\n\tif cached := c.subtypeReductionCache[key]; cached != nil {\n\t\tAgentOracleCounters["subtypeHitArguments"] += len(types)\n\t\treturn cached\n\t}\n\tAgentOracleCounters["subtypeMissArguments"] += len(types)');
    }
    if (request.inspectActiveMapperCaches === true) {
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) popActiveMapper() {',
        'func (c *Checker) popActiveMapper() {\n\tAgentOracleCounters["activeMapperPopFrames"]++\n\tAgentOracleCounters["activeMapperPopEntries"] += len(c.activeTypeMappersCaches[len(c.activeTypeMappersCaches)-1])');
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) clearActiveMapperCaches() {\n\tfor _, cache := range c.activeTypeMappersCaches {',
        'func (c *Checker) clearActiveMapperCaches() {\n\tAgentOracleCounters["activeMapperClearCalls"]++\n\tfor _, cache := range c.activeTypeMappersCaches {\n\t\tAgentOracleCounters["activeMapperClearFrames"]++\n\t\tAgentOracleCounters["activeMapperClearEntries"] += len(cache)');
      // Count the existing alias key traversal, without forcing other types.
      checkerSource = replaceExactly(checkerSource,
        '\tcache := c.activeTypeMappersCaches[core.IfElse(index != -1, index, len(c.activeTypeMappersCaches)-1)]',
        '\tAgentOracleCounters["activeMapperLookups"]++\n\tif alias != nil { AgentOracleCounters["activeMapperAliasArguments"] += len(alias.typeArguments) }\n\tcache := c.activeTypeMappersCaches[core.IfElse(index != -1, index, len(c.activeTypeMappersCaches)-1)]');
    }
    if (request.inspectTupleTargetCaches === true) {
      // Observe the existing getTupleTargetType key, including original labels.
      checkerSource = replaceExactly(checkerSource,
        '\tkey := getTupleKey(elementInfos, readonly)\n\tt := c.tupleTypes[key]',
        '\tkey := getTupleKey(elementInfos, readonly)\n\tAgentOracleCounters["tupleTargetLookups"]++\n\tAgentOracleCounters["tupleTargetElements"] += len(elementInfos)\n\tt := c.tupleTypes[key]\n\tif t == nil { AgentOracleCounters["tupleTargetMisses"]++\n\tAgentOracleCounters["tupleTargetMissElements"] += len(elementInfos) } else { AgentOracleCounters["tupleTargetHits"]++\n\tAgentOracleCounters["tupleTargetHitElements"] += len(elementInfos) }');
    }
    if (request.inspectSignatureInstantiationCaches === true) {
      // Observe the actual signature key after argument filling has finished.
      checkerSource = replaceExactly(checkerSource,
        '\tkey := CachedSignatureKey{sig: sig, key: getTypeListKey(typeArguments)}\n\tinstantiation := c.cachedSignatures[key]',
        '\tkey := CachedSignatureKey{sig: sig, key: getTypeListKey(typeArguments)}\n\tAgentOracleCounters["signatureInstantiationLookups"]++\n\tAgentOracleCounters["signatureInstantiationArguments"] += len(typeArguments)\n\tinstantiation := c.cachedSignatures[key]\n\tif instantiation == nil { AgentOracleCounters["signatureInstantiationMisses"]++ } else { AgentOracleCounters["signatureInstantiationHits"]++\n\tAgentOracleCounters["signatureInstantiationHitArguments"] += len(typeArguments) }');
    }
    if (request.inspectConditionalInstantiationCaches === true) {
      // Observe getConditionalTypeInstantiation after native outer mapping.
      checkerSource = replaceExactly(checkerSource,
        '\t\tkey := getConditionalTypeKey(typeArguments, alias, forConstraint)\n\t\tresult := root.instantiations[key]',
        '\t\tkey := getConditionalTypeKey(typeArguments, alias, forConstraint)\n\t\tAgentOracleCounters["conditionalInstantiationLookups"]++\n\t\tAgentOracleCounters["conditionalInstantiationArguments"] += len(typeArguments)\n\t\tresult := root.instantiations[key]\n\t\tif result == nil { AgentOracleCounters["conditionalInstantiationMisses"]++ } else { AgentOracleCounters["conditionalInstantiationHits"]++\n\t\tAgentOracleCounters["conditionalInstantiationHitArguments"] += len(typeArguments) }');
    }
    if (request.inspectObjectInstantiationCaches === true) {
      // Count existing effective argument keys, retaining native cache barriers.
      checkerSource = replaceExactly(checkerSource,
        '\tresult := data.instantiations[key]',
        '\tAgentOracleCounters["objectInstantiationLookups"]++\n\tAgentOracleCounters["objectInstantiationArguments"] += len(typeArguments)\n\tresult := data.instantiations[key]\n\tif result == nil { AgentOracleCounters["objectInstantiationMisses"]++ } else { AgentOracleCounters["objectInstantiationHits"]++\n\tAgentOracleCounters["objectInstantiationHitArguments"] += len(typeArguments) }');
    }
    if (request.inspectAliasCaches === true) {
      // Observe the existing raw-argument cache before defaults are filled.
      checkerSource = replaceExactly(checkerSource,
        '\tkey := getTypeAliasInstantiationKey(typeArguments, alias)\n\tinstantiation := links.instantiations[key]',
        '\tkey := getTypeAliasInstantiationKey(typeArguments, alias)\n\tAgentOracleCounters["aliasLookups"]++\n\tAgentOracleCounters["aliasKeyArguments"] += len(typeArguments)\n\tinstantiation := links.instantiations[key]\n\tif instantiation == nil { AgentOracleCounters["aliasMisses"]++ } else { AgentOracleCounters["aliasHits"]++\n\tAgentOracleCounters["aliasHitArguments"] += len(typeArguments) }');
    }
    if (request.inspectErrorAliasCaches === true) {
      // Observe unresolved-alias cache outcomes without acquiring other types.
      checkerSource = replaceExactly(checkerSource,
        '\t\tkey := getAliasKey(alias)\n\t\terrorType := c.errorTypes[key]',
        '\t\tkey := getAliasKey(alias)\n\t\tAgentOracleCounters["errorAliasLookups"]++\n\t\tAgentOracleCounters["errorAliasArguments"] += len(alias.typeArguments)\n\t\terrorType := c.errorTypes[key]\n\t\tif errorType == nil { AgentOracleCounters["errorAliasMisses"]++ } else { AgentOracleCounters["errorAliasHits"]++\n\t\tAgentOracleCounters["errorAliasHitArguments"] += len(alias.typeArguments) }');
    }
    if (request.inspectIndexedCaches === true) {
      // Observe established operands and actual cache outcomes only.
      checkerSource = replaceExactly(checkerSource,
        '\t\tkey := getIndexedAccessKey(objectType, indexType, accessFlags, alias)\n\t\tt := c.indexedAccessTypes[key]',
        '\t\tkey := getIndexedAccessKey(objectType, indexType, accessFlags, alias)\n\t\tAgentOracleCounters["indexedLookups"]++\n\t\tif alias != nil { AgentOracleCounters["indexedAliasArguments"] += len(alias.typeArguments) }\n\t\tt := c.indexedAccessTypes[key]\n\t\tif t == nil { AgentOracleCounters["indexedMisses"]++ } else { AgentOracleCounters["indexedHits"]++ }');
    }
    if (request.inspectUnionCaches === true) {
      // Observe existing cache outcomes without acquiring another type.
      checkerSource = replaceExactly(checkerSource,
        '\tkey := getUnionKey(types, origin, alias)\n\tt := c.unionTypes[key]',
        '\tkey := getUnionKey(types, origin, alias)\n\tAgentOracleCounters["unionLookups"]++\n\tAgentOracleCounters["unionKeyTypes"] += len(types)\n\tt := c.unionTypes[key]\n\tif t == nil { AgentOracleCounters["unionMisses"]++ } else { AgentOracleCounters["unionHits"]++\n\tAgentOracleCounters["unionHitTypes"] += len(types) }');
    }
    if (request.inspectIntersectionResources === true) {
      // Count existing visits and cache outcomes without asking new questions.
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) addTypeToIntersection(typeSet *orderedSet[*Type], includes TypeFlags, t *Type) TypeFlags {',
        'func (c *Checker) addTypeToIntersection(typeSet *orderedSet[*Type], includes TypeFlags, t *Type) TypeFlags {\n\tAgentOracleCounters["intersectionVisits"]++\n\tif t.flags&TypeFlagsIntersection == 0 { AgentOracleCounters["intersectionLeaves"]++ }');
      checkerSource = replaceExactly(checkerSource,
        '\tkey := getIntersectionKey(typeSet, flags, alias)\n\tresult := c.intersectionTypes[key]',
        '\tkey := getIntersectionKey(typeSet, flags, alias)\n\tAgentOracleCounters["intersectionLookups"]++\n\tAgentOracleCounters["intersectionKeyTypes"] += len(typeSet)\n\tif alias != nil { AgentOracleCounters["intersectionAliasArguments"] += len(alias.typeArguments) }\n\tresult := c.intersectionTypes[key]\n\tif result != nil { AgentOracleCounters["intersectionHits"]++\n\tAgentOracleCounters["intersectionHitTypes"] += len(typeSet) }');
    }
    if (request.inspectReferenceCaches === true) {
      // Observe existing factory calls and cache outcomes; acquire no types.
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) createTypeReferenceEx(target *Type, typeArguments []*Type, objectFlags ObjectFlags) *Type {',
        'func (c *Checker) createTypeReferenceEx(target *Type, typeArguments []*Type, objectFlags ObjectFlags) *Type {\n\tAgentOracleCounters["referenceLookups"]++\n\tAgentOracleCounters["referenceArguments"] += len(typeArguments)');
      checkerSource = replaceExactly(checkerSource,
        '\tif t, ok := intf.instantiations[id]; ok {\n\t\treturn t',
        '\tif t, ok := intf.instantiations[id]; ok {\n\t\tAgentOracleCounters["referenceHits"]++\n\t\tAgentOracleCounters["referenceHitArguments"] += len(typeArguments)\n\t\treturn t');
    }
    if (request.inspectConstantTemplates === true) {
      const evaluatorPath = path.join(sourceDirectory, 'internal/evaluator/evaluator.go');
      let evaluatorSource = fs.readFileSync(evaluatorPath, 'utf8');
      evaluatorSource = replaceExactly(evaluatorSource,
        'func evaluateTemplateExpression(expr *ast.Node, location *ast.Node, evaluate Evaluator) Result {',
        'var AgentOracleTemplateSpanVisits int\n\nfunc evaluateTemplateExpression(expr *ast.Node, location *ast.Node, evaluate Evaluator) Result {');
      evaluatorSource = replaceExactly(evaluatorSource,
        '\tfor _, span := range expr.AsTemplateExpression().TemplateSpans.Nodes {',
        '\tfor _, span := range expr.AsTemplateExpression().TemplateSpans.Nodes {\n\t\tAgentOracleTemplateSpanVisits++');
      const overlaidEvaluator = path.join(temporary, 'evaluator.go');
      fs.writeFileSync(overlaidEvaluator, evaluatorSource);
      attach(overlaidEvaluator, 'internal/evaluator/evaluator.go');
      checkerSource = replaceExactly(checkerSource,
        '\tresult := c.evaluate(initializer, member)',
        '\tresult := c.evaluate(initializer, member)\n\tAgentOracleCounters["constantTemplateSpans"] = evaluator.AgentOracleTemplateSpanVisits');
      checkerSource = replaceExactly(checkerSource,
        'func (c *Checker) checkTemplateExpression(node *ast.Node) *Type {',
        'func (c *Checker) checkTemplateExpression(node *ast.Node) *Type {\n\tAgentOracleCounters["checkedTemplateExpressions"]++');
    }
    if (request.inspectTemplateCaches === true) {
      checkerSource = replaceExactly(checkerSource,
        '\taddSpans = func(texts []string, types []*Type) bool {\n\t\tfor i, t := range types {',
        '\taddSpans = func(texts []string, types []*Type) bool {\n\t\tfor i, t := range types {\n\t\t\tAgentOracleCounters["templateSpanVisits"]++');
      checkerSource = replaceExactly(checkerSource,
        '\tkey := getTemplateTypeKey(newTexts, newTypes)\n\tt := c.templateLiteralTypes[key]',
        '\tAgentOracleCounters["templateCacheLookups"]++\n\tAgentOracleCounters["templateCacheTypes"] += len(newTypes)\n\tfor _, text := range newTexts { AgentOracleCounters["templateCacheBytes"] += len(text) }\n\tkey := getTemplateTypeKey(newTexts, newTypes)\n\tt := c.templateLiteralTypes[key]\n\tif t == nil { AgentOracleCounters["templateCacheMisses"]++ }');
    }
    if (request.inspectTupleNormalization === true) {
      checkerSource = replaceExactly(checkerSource,
        '\tnormalizedElements := core.SameMap(elements, func(t *Type) *Type {',
        '\tnormalizedElements := core.SameMap(elements, func(t *Type) *Type {\n\t\tAgentOracleCounters["tupleNormalizationElements"]++');
    }
    if (request.inspectTupleEdges === true) {
      checkerSource = replaceExactly(checkerSource,
        'func getStartElementCount(t *TupleType, flags ElementFlags) int {\n\tfor i, info := range t.elementInfos {',
        'func getStartElementCount(t *TupleType, flags ElementFlags) int {\n\tfor i, info := range t.elementInfos {\n\t\tAgentOracleCounters["tupleStartVisits"]++');
      checkerSource = replaceExactly(checkerSource,
        'func getEndElementCount(t *TupleType, flags ElementFlags) int {\n\tfor i := len(t.elementInfos); i > 0; i-- {',
        'func getEndElementCount(t *TupleType, flags ElementFlags) int {\n\tfor i := len(t.elementInfos); i > 0; i-- {\n\t\tAgentOracleCounters["tupleEndVisits"]++');
    }
    const overlaidChecker = path.join(temporary, 'checker.go');
    fs.writeFileSync(overlaidChecker, checkerSource);
    attach(overlaidChecker, 'internal/checker/checker.go');
    const nodeBuilderPath = path.join(sourceDirectory, 'internal/checker/nodebuilderimpl.go');
    let nodeBuilderSource = fs.readFileSync(nodeBuilderPath, 'utf8');
    nodeBuilderSource = replaceExactly(nodeBuilderSource,
      '\tkey := CompositeTypeCacheIdentity{typeId, b.ctx.flags, b.ctx.internalFlags}',
      '\tkey := CompositeTypeCacheIdentity{typeId, b.ctx.flags, b.ctx.internalFlags}\n\tb.agentOracleDiagnosticVisit("visit", t, key, nil, -1)');
    nodeBuilderSource = replaceExactly(nodeBuilderSource,
      '\t\tif ok {\n\t\t\t// TODO:: check if we instead store late painted statements associated with this?',
      '\t\tif ok {\n\t\t\tb.agentOracleDiagnosticVisit("hit", t, key, cachedResult, -1)\n\t\t\t// TODO:: check if we instead store late painted statements associated with this?');
    nodeBuilderSource = replaceExactly(nodeBuilderSource,
      '\t\t\tb.ctx.approximateLength += cachedResult.addedLength\n\t\t\treturn b.f.DeepCloneNode(cachedResult.node)',
      '\t\t\tb.ctx.approximateLength += cachedResult.addedLength\n\t\t\tb.agentOracleDiagnosticVisit("replayed", t, key, cachedResult, -1)\n\t\t\treturn b.f.DeepCloneNode(cachedResult.node)');
    nodeBuilderSource = replaceExactly(nodeBuilderSource,
      '\tvar depth int\n\tif id != nil {\n\t\tdepth = b.ctx.symbolDepth[*id]',
      '\tb.agentOracleDiagnosticVisit("miss", t, key, nil, -1)\n\tvar depth int\n\tif id != nil {\n\t\tdepth = b.ctx.symbolDepth[*id]');
    nodeBuilderSource = replaceExactly(nodeBuilderSource,
      '\t\tif depth > 10 {\n\t\t\treturn b.createElidedInformationPlaceholder()',
      '\t\tif depth > 10 {\n\t\t\tb.agentOracleDiagnosticVisit("depthElision", t, key, nil, depth)\n\t\t\treturn b.createElidedInformationPlaceholder()');
    nodeBuilderSource = replaceExactly(nodeBuilderSource,
      '\tresult := transform(b, t)\n\taddedLength :=',
      '\tresult := transform(b, t)\n\tb.agentOracleDiagnosticVisit("transformed", t, key, nil, depth)\n\taddedLength :=');
    nodeBuilderSource = replaceExactly(nodeBuilderSource,
      '\t\t\ttrackedSymbols: b.ctx.trackedSymbols,\n\t\t}\n\t}\n\tb.ctx.visitedTypes.Delete(typeId)',
      '\t\t\ttrackedSymbols: b.ctx.trackedSymbols,\n\t\t}\n\t\tb.agentOracleDiagnosticVisit("installed", t, key, links.serializedTypes[key], depth)\n\t} else {\n\t\tb.agentOracleDiagnosticVisit("notInstalled", t, key, nil, depth)\n\t}\n\tb.ctx.visitedTypes.Delete(typeId)');
    nodeBuilderSource = replaceExactly(nodeBuilderSource,
      '\tb.ctx.trackedSymbols = prevTrackedSymbols\n\treturn result',
      '\tb.ctx.trackedSymbols = prevTrackedSymbols\n\tb.agentOracleDiagnosticVisit("cleaned", t, key, nil, depth)\n\treturn result');
    const overlaidNodeBuilder = path.join(temporary, 'nodebuilderimpl.go');
    fs.writeFileSync(overlaidNodeBuilder, nodeBuilderSource);
    attach(overlaidNodeBuilder, 'internal/checker/nodebuilderimpl.go');
    const symbolTrackerPath = path.join(sourceDirectory, 'internal/checker/symboltracker.go');
    const symbolTrackerSource = replaceExactly(fs.readFileSync(symbolTrackerPath, 'utf8'),
      'func (this *SymbolTrackerImpl) TrackSymbol(symbol *ast.Symbol, enclosingDeclaration *ast.Node, meaning ast.SymbolFlags) bool {',
      'func (this *SymbolTrackerImpl) TrackSymbol(symbol *ast.Symbol, enclosingDeclaration *ast.Node, meaning ast.SymbolFlags) bool {\n\tagentOracleDiagnosticTrack(this.context, symbol, enclosingDeclaration, meaning, this.DisableTrackSymbol)');
    const overlaidSymbolTracker = path.join(temporary, 'symboltracker.go');
    fs.writeFileSync(overlaidSymbolTracker, symbolTrackerSource);
    attach(overlaidSymbolTracker, 'internal/checker/symboltracker.go');
    const utilitiesPath = path.join(sourceDirectory, 'internal/checker/utilities.go');
    const utilitiesSource = replaceExactly(fs.readFileSync(utilitiesPath, 'utf8'),
      'func getDeclarationModifierFlagsFromSymbolEx(s *ast.Symbol, isWrite bool) ast.ModifierFlags {',
      'func getDeclarationModifierFlagsFromSymbolEx(s *ast.Symbol, isWrite bool) (result ast.ModifierFlags) {\n\tdefer func() {\n\t\tif s.Flags&ast.SymbolFlagsPrototype != 0 { AgentOracleObservation("prototypeModifiers", map[string]any{"flags": uint32(result), "writing": isWrite}) }\n\t}()');
    const overlaidUtilities = path.join(temporary, 'utilities.go');
    fs.writeFileSync(overlaidUtilities, utilitiesSource);
    attach(overlaidUtilities, 'internal/checker/utilities.go');
    const flowPath = path.join(sourceDirectory, 'internal/checker/flow.go');
    let flowSource = replaceExactly(fs.readFileSync(flowPath, 'utf8'), '\tif f.depth == 2000 {',
      '\tAgentOracleCounter("flowDepth", f.depth)\n\tif f.depth == 2000 {\n\t\tAgentOracleFailure("error_type")');
    flowSource = replaceExactly(flowSource,
      'func (c *Checker) getFlowTypeOfReferenceEx(reference *ast.Node, declaredType *Type, initialType *Type, flowContainer *ast.Node, flowNode *ast.FlowNode) *Type {',
      'func (c *Checker) getFlowTypeOfReferenceEx(reference *ast.Node, declaredType *Type, initialType *Type, flowContainer *ast.Node, flowNode *ast.FlowNode) (result *Type) {\n\tdefer func() {\n\t\tkey := "referenceFlow:" + strconv.Itoa(reference.Pos()) + ":" + strconv.Itoa(reference.End())\n\t\tAgentOracleObservation(key, map[string]any{"referencePos": reference.Pos(), "referenceEnd": reference.End(), "sameDeclared": result == declaredType, "sameInitial": result == initialType, "errorType": c.isErrorType(result), "declaredFlags": uint32(declaredType.flags), "resultFlags": uint32(result.flags)})\n\t}()');
    flowSource = replaceExactly(flowSource,
      'func (c *Checker) getTypeOfDestructuredProperty(t *Type, name *ast.Node) *Type {',
      'func (c *Checker) getTypeOfDestructuredProperty(t *Type, name *ast.Node) (result *Type) {\n\tdefer func() {\n\t\tAgentOracleObservation("destructuredPropertyReturn", map[string]any{"errorType": c.isErrorType(result)})\n\t}()');
    // Observe the cache and already-established values without evaluating a
    // second relation, projection, default expression, or type-ID allocator.
    flowSource = replaceExactly(flowSource,
      '\tresult := c.assignmentReducedTypes[key]\n\tif result == nil {\n\t\tresult = c.getAssignmentReducedTypeWorker(declaredType, assignedType)\n\t\tc.assignmentReducedTypes[key] = result\n\t}',
      '\tresult := c.assignmentReducedTypes[key]\n\tAgentOracleCounters["assignment"]++\n\tAgentOracleObservation("assignment:" + strconv.Itoa(AgentOracleCounters["assignment"]), map[string]any{"event": "lookup", "declared": declaredType.id, "assigned": assignedType.id, "hit": result != nil})\n\tif result == nil {\n\t\tresult = c.getAssignmentReducedTypeWorker(declaredType, assignedType)\n\t\tc.assignmentReducedTypes[key] = result\n\t\tAgentOracleCounters["assignment"]++\n\t\tAgentOracleObservation("assignment:" + strconv.Itoa(AgentOracleCounters["assignment"]), map[string]any{"event": "installed", "declared": declaredType.id, "assigned": assignedType.id, "sameDeclared": result == declaredType, "sameAssigned": result == assignedType, "errorType": result == c.errorType})\n\t}');
    flowSource = replaceExactly(flowSource,
      'func (c *Checker) getTypeWithDefault(t *Type, defaultExpression *ast.Node) *Type {',
      'func (c *Checker) getTypeWithDefault(t *Type, defaultExpression *ast.Node) (result *Type) {\n\tif defaultExpression != nil {\n\t\tAgentOracleCounters["assignment"]++\n\t\tAgentOracleObservation("assignment:" + strconv.Itoa(AgentOracleCounters["assignment"]), map[string]any{"event": "defaultEnter", "pos": defaultExpression.Pos(), "end": defaultExpression.End(), "inputError": t == c.errorType, "inputAny": t != nil && t.flags&TypeFlagsAny != 0})\n\t\tdefer func() {\n\t\t\tAgentOracleCounters["assignment"]++\n\t\t\tAgentOracleObservation("assignment:" + strconv.Itoa(AgentOracleCounters["assignment"]), map[string]any{"event": "defaultExit", "pos": defaultExpression.Pos(), "end": defaultExpression.End(), "sameInput": result == t, "errorType": result == c.errorType, "resultAny": result != nil && result.flags&TypeFlagsAny != 0})\n\t\t}()\n\t}');
    const overlaidFlow = path.join(temporary, 'flow.go');
    fs.writeFileSync(overlaidFlow, flowSource);
    attach(overlaidFlow, 'internal/checker/flow.go');
    const relaterPath = path.join(sourceDirectory, 'internal/checker/relater.go');
    let relaterSource = fs.readFileSync(relaterPath, 'utf8');
    relaterSource = replaceExactly(relaterSource,
      'func (c *Checker) computeKeyPropertyNameAndMap(t *Type) (string, map[*Type]*Type) {',
      'func (c *Checker) computeKeyPropertyNameAndMap(t *Type) (name string, mapping map[*Type]*Type) {\n\tdefer func() {\n\t\tambiguous := 0\n\t\tfor _, value := range mapping { if value == c.unknownType { ambiguous++ } }\n\t\tAgentOracleObservation("unionKey:" + strconv.Itoa(int(t.id)), map[string]any{"constituents": len(t.Types()), "tagName": name == "tag", "hasMap": mapping != nil, "entries": len(mapping), "ambiguous": ambiguous})\n\t}()');
    relaterSource = replaceExactly(relaterSource, '\tr.relationCount = (16_000_000 - relation.size()) / 8',
      '\tr.relationCount = (16_000_000 - relation.size()) / 8\n\tAgentOracleCounter("relationInitialAllowance", r.relationCount)');
    relaterSource = replaceExactly(relaterSource, '\tresult := r.isRelatedToEx(source, target, RecursionFlagsBoth, errorNode != nil /*reportErrors*/, headMessage, IntersectionStateNone)',
      '\tresult := r.isRelatedToEx(source, target, RecursionFlagsBoth, errorNode != nil /*reportErrors*/, headMessage, IntersectionStateNone)\n\tAgentOracleRelationFinish(r.relationCount, relation.size(), int(result), r.overflow)');
    relaterSource = replaceExactly(relaterSource, '\tif r.relationCount <= 0 {',
      '\tAgentOracleCounter("relationSourceDepth", len(r.sourceStack))\n\tAgentOracleCounter("relationTargetDepth", len(r.targetStack))\n\tAgentOracleCounter("relationAllowanceUsed", 2_000_000-r.relationCount)\n\tif r.relationCount <= 0 {\n\t\tAgentOracleFailure("relation_complexity_limit")');
    relaterSource = replaceExactly(relaterSource, '\tif r.maybeKeysSet.Has(id) {',
      '\tif r.maybeKeysSet.Has(id) {\n\t\tAgentOracleObservation("activeKeyMaybe", map[string]any{"sourceDepth": len(r.sourceStack), "targetDepth": len(r.targetStack), "allowance": r.relationCount})');
    relaterSource = replaceExactly(relaterSource, '\t\tif r.maybeKeysSet.Has(broadestEquivalentId) {',
      '\t\tif r.maybeKeysSet.Has(broadestEquivalentId) {\n\t\t\tAgentOracleObservation("constrainedKeyMaybe", map[string]any{"sourceDepth": len(r.sourceStack), "targetDepth": len(r.targetStack), "allowance": r.relationCount})');
    relaterSource = replaceExactly(relaterSource, '\tif len(r.sourceStack) == 100 || len(r.targetStack) == 100 {',
      '\tif len(r.sourceStack) == 100 || len(r.targetStack) == 100 {\n\t\tAgentOracleFailure("relation_depth_limit")');
    relaterSource = replaceExactly(relaterSource, '\tif r.expandingFlags == ExpandingFlagsBoth {',
      '\tif r.expandingFlags == ExpandingFlagsBoth {\n\t\tAgentOracleObservation("deeplyNestedMaybe", map[string]any{"sourceDepth": len(r.sourceStack), "targetDepth": len(r.targetStack), "allowance": r.relationCount})');
    if (request.inspectRelationProperties === true) {
      relaterSource = replaceExactly(relaterSource,
        'func (c *Checker) getUnmatchedPropertiesWorker(source *Type, target *Type, requireOptionalProperties bool, matchDiscriminantProperties bool, propsOut *[]*ast.Symbol) *ast.Symbol {',
        'func (c *Checker) getUnmatchedPropertiesWorker(source *Type, target *Type, requireOptionalProperties bool, matchDiscriminantProperties bool, propsOut *[]*ast.Symbol) *ast.Symbol {\n\tif propsOut != nil {\n\t\tAgentOracleCounters["unmatchedFullCalls"]++\n\t\tdefer func() { AgentOracleCounters["unmatchedRetained"] += len(*propsOut) }()\n\t}');
      relaterSource = replaceExactly(relaterSource,
        'func (r *Relater) isRelatedToEx(originalSource *Type, originalTarget *Type, recursionFlags RecursionFlags, reportErrors bool, headMessage *diagnostics.Message, intersectionState IntersectionState) Ternary {',
        'func (r *Relater) isRelatedToEx(originalSource *Type, originalTarget *Type, recursionFlags RecursionFlags, reportErrors bool, headMessage *diagnostics.Message, intersectionState IntersectionState) Ternary {\n\tAgentOracleCounters["relationEntry"]++\n\tAgentOracleObservation("relationEntry:" + strconv.Itoa(AgentOracleCounters["relationEntry"]), map[string]any{"source": originalSource.id, "target": originalTarget.id, "sourceFlags": uint32(originalSource.flags), "targetFlags": uint32(originalTarget.flags), "same": originalSource == originalTarget})');
      relaterSource = replaceExactly(relaterSource,
        '\t\t\tif sourceProp != nil && sourceProp != targetProp {',
        '\t\t\tAgentOracleCounters["relationPropertyVisits"]++\n\t\t\tif sourceProp == targetProp { AgentOracleCounters["relationPropertyIdentitySkips"]++ }\n\t\t\tif sourceProp != nil && sourceProp != targetProp {');
    }
    if (request.inspectRelationStacks === true) {
      for (const side of ['source', 'target']) {
        const name = side === 'source' ? 'Source' : 'Target';
        relaterSource = replaceExactly(relaterSource,
          `\t\tr.${side}Stack = append(r.${side}Stack, ${side})`,
          `\t\tr.${side}Stack = append(r.${side}Stack, ${side})\n\t\tAgentOracleCounters["relation${name}StackEntries"] += len(r.${side}Stack)\n\t\tAgentOracleCounters["relation${name}StackPeak"] = max(AgentOracleCounters["relation${name}StackPeak"], len(r.${side}Stack))`);
      }
    }
    if (request.inspectRelationCache === true) {
      relaterSource = replaceExactly(relaterSource,
        '\t\t\tr.relation.set(r.maybeKeys[i], RelationComparisonResultSucceeded|propagatingVarianceFlags)',
        '\t\t\tr.relation.set(r.maybeKeys[i], RelationComparisonResultSucceeded|propagatingVarianceFlags)\n\t\t\tAgentOracleCounters["relationCacheCommits"]++\n\t\t\tAgentOracleCounters["relationCacheEntries"] = r.relation.size()');
    }
    if (request.inspectRelationArguments === true) {
      relaterSource = replaceExactly(relaterSource,
        '\t\tvariance := varianceFlags & VarianceFlagsVarianceMask',
        '\t\tvariance := varianceFlags & VarianceFlagsVarianceMask\n\t\tAgentOracleCounters["relationArgumentVisits"]++\n\t\tif variance == VarianceFlagsIndependent { AgentOracleCounters["relationIndependentArguments"]++ }');
    }
    if (request.inspectRelationTuples === true) {
      relaterSource = replaceExactly(relaterSource,
        '\t\t\tfor sourcePosition := range sourceArity {',
        '\t\t\tfor sourcePosition := range sourceArity {\n\t\t\t\tAgentOracleCounters["relationTupleVisits"]++');
    }
    if (request.inspectStringComparisons === true) {
      relaterSource = replaceExactly(relaterSource,
        'func (c *Checker) inferFromLiteralPartsToTemplateLiteral(sourceTexts []string, sourceTypes []*Type, target *TemplateLiteralType) []*Type {',
        'func (c *Checker) inferFromLiteralPartsToTemplateLiteral(sourceTexts []string, sourceTypes []*Type, target *TemplateLiteralType) []*Type {\n\tAgentOracleCounters["literalPartsMatches"]++\n\tAgentOracleCounters["literalPartsPrefixBytes"] += len(target.texts[0])\n\tAgentOracleCounters["literalPartsSuffixBytes"] += len(target.texts[len(target.texts)-1])');
    }
    if (request.inspectSignatureArityResources === true) {
      // Observe the installed native arity slot; syntactic-only arity deliberately
      // neither reads nor publishes that slot. No extra checker question.
      relaterSource = replaceExactly(relaterSource,
        "func (c *Checker) getMinArgumentCountEx(signature *Signature, flags MinArgumentCountFlags) int {",
        "func (c *Checker) getMinArgumentCountEx(signature *Signature, flags MinArgumentCountFlags) int {\n\tdefer func() {\n\t\tif flags&MinArgumentCountFlagsVoidIsNonOptional == 0 && signature.declaration != nil && signature.declaration.Name() != nil && strings.HasPrefix(signature.declaration.Name().Text(), \"CachedCall\") && signature.resolvedMinArgumentCount >= 0 { AgentOracleObservation(\"signatureArity:\"+signature.declaration.Name().Text(), map[string]any{\"minimum\": signature.resolvedMinArgumentCount}) }\n\t}()");
    }
    const overlaidRelater = path.join(temporary, 'relater.go');
    fs.writeFileSync(overlaidRelater, relaterSource);
    attach(overlaidRelater, 'internal/checker/relater.go');
    const referencesPath = path.join(sourceDirectory, 'internal/ls/findallreferences.go');
    const referencesSource = replaceExactly(fs.readFileSync(referencesPath, 'utf8'),
      '\t\tstate.getReferencesInContainerOrFiles(symbol, search)',
      '\t\tagentOracleAfterSearchSet(state, search)\n\t\tstate.getReferencesInContainerOrFiles(symbol, search)');
    const overlaidReferences = path.join(temporary, 'findallreferences.go');
    fs.writeFileSync(overlaidReferences, referencesSource);
    attach(overlaidReferences, 'internal/ls/findallreferences.go');
    // Optional zero-demand flow evidence: count getTypeAtFlowNode loop
    // iterations, shared-cache hits/records, entries and depth for each
    // getFlowTypeOfReferenceEx invocation. Existing fields only are read.
    if (request.inspectFlowNodes === true || request.inspectReachability === true) {
      const flowPath = path.join(sourceDirectory, 'internal/checker/flow.go');
      let flowSource = fs.readFileSync(flowPath, 'utf8');
      flowSource = replaceExactly(flowSource,
        '\tc.flowInvocationCount++\n\tevolvedType := c.getTypeAtFlowNode(f, flowNode).t\n\tc.sharedFlows = c.sharedFlows[:f.sharedFlowStart]\n\tc.putFlowState(f)\n',
        '\tc.flowInvocationCount++\n\tagentOracleFlowKeys[f] = "flowNodes:" + strconv.Itoa(c.flowInvocationCount)\n\tAgentOracleObservation(agentOracleFlowKeys[f], map[string]any{"visits": 0, "sharedHits": 0, "sharedRecords": 0, "entries": 0, "maxDepth": 0, "referenceKind": reference.Kind, "referencePos": reference.Pos(), "referenceEnd": reference.End()})\n\tevolvedType := c.getTypeAtFlowNode(f, flowNode).t\n\tdelete(agentOracleFlowKeys, f)\n\tc.sharedFlows = c.sharedFlows[:f.sharedFlowStart]\n\tc.putFlowState(f)\n');
      flowSource = replaceExactly(flowSource,
        '\tf.depth++\n\tvar sharedFlow *ast.FlowNode\n\tfor {\n\t\tflags := flow.Flags\n',
        '\tf.depth++\n\tagentFlowRow := AgentOracleObservations[agentOracleFlowKeys[f]]\n\tif agentFlowRow != nil {\n\t\tagentFlowRow["entries"] = agentFlowRow["entries"].(int) + 1\n\t\tif f.depth > agentFlowRow["maxDepth"].(int) { agentFlowRow["maxDepth"] = f.depth }\n\t}\n\tvar sharedFlow *ast.FlowNode\n\tfor {\n\t\tif agentFlowRow != nil { agentFlowRow["visits"] = agentFlowRow["visits"].(int) + 1 }\n\t\tflags := flow.Flags\n');
      flowSource = replaceExactly(flowSource,
        '\t\t\t\tif c.sharedFlows[i].flow == flow {\n\t\t\t\t\tf.depth--\n',
        '\t\t\t\tif c.sharedFlows[i].flow == flow {\n\t\t\t\t\tif agentFlowRow != nil { agentFlowRow["sharedHits"] = agentFlowRow["sharedHits"].(int) + 1 }\n\t\t\t\t\tf.depth--\n');
      flowSource = replaceExactly(flowSource,
        '\t\tif sharedFlow != nil {\n\t\t\t// Record visited node and the associated type in the cache.\n',
        '\t\tif sharedFlow != nil {\n\t\t\tif agentFlowRow != nil { agentFlowRow["sharedRecords"] = agentFlowRow["sharedRecords"].(int) + 1 }\n\t\t\t// Record visited node and the associated type in the cache.\n');
      flowSource += '\nvar agentOracleFlowKeys = map[*FlowState]string{}\n';
      if (request.inspectReachability === true) {
        flowSource = replaceExactly(flowSource,
          'func (c *Checker) isReachableFlowNodeWorker(f *FlowState, flow *ast.FlowNode, noCacheCheck bool) bool {\n\tfor {',
          'func (c *Checker) isReachableFlowNodeWorker(f *FlowState, flow *ast.FlowNode, noCacheCheck bool) bool {\n\tAgentOracleCounters["reachabilityEntries"]++\n\tfor {\n\t\tAgentOracleCounters["reachabilityVisits"]++');
        flowSource = replaceExactly(flowSource,
          '\t\t\t\tif reachable, ok := c.flowNodeReachable[flow]; ok {',
          '\t\t\t\tif reachable, ok := c.flowNodeReachable[flow]; ok {\n\t\t\t\t\tAgentOracleCounters["reachabilityHits"]++');
        flowSource = replaceExactly(flowSource,
          '\t\t\t\tc.flowNodeReachable[flow] = reachable',
          '\t\t\t\tc.flowNodeReachable[flow] = reachable\n\t\t\t\tAgentOracleCounters["reachabilityRecords"]++');
      }
      const overlaidFlow = path.join(temporary, 'flow.go');
      fs.writeFileSync(overlaidFlow, flowSource);
      attach(overlaidFlow, 'internal/checker/flow.go');
    }
    // Count actual mapper calls and nesting without adding a type demand or
    // re-running a source/target comparison. The native Map body is unchanged.
    if (request.inspectMappers === true) {
      const mapperPath = path.join(sourceDirectory, 'internal/checker/mapper.go');
      const kinds = ['SimpleTypeMapper', 'ArrayTypeMapper', 'ArrayToSingleTypeMapper',
        'DeferredTypeMapper', 'FunctionTypeMapper', 'MergedTypeMapper', 'CompositeTypeMapper',
        'InferenceTypeMapper'];
      const mapperSource = replaceExactly(fs.readFileSync(mapperPath, 'utf8'),
        'func (m *TypeMapper) Map(t *Type) *Type    { return m.data.Map(t) }',
        `func (m *TypeMapper) Map(t *Type) *Type {
    AgentOracleCounters["mapperDepth"]++
    if AgentOracleCounters["mapperDepth"] > AgentOracleCounters["mapperMaxDepth"] {
        AgentOracleCounters["mapperMaxDepth"] = AgentOracleCounters["mapperDepth"]
    }
    defer func() { AgentOracleCounters["mapperDepth"]-- }()
    switch m.data.(type) {
${kinds.map(kind => `    case *${kind}: AgentOracleCounters["mapper:${kind}"]++`).join('\n')}
    }
    return m.data.Map(t)
}`);
      const overlaidMapper = path.join(temporary, 'mapper.go');
      fs.writeFileSync(overlaidMapper, mapperSource);
      attach(overlaidMapper, 'internal/checker/mapper.go');
    }
    const overlay = path.join(temporary, 'overlay.json'), inputFile = path.join(temporary, 'input.json'), outputFile = path.join(temporary, 'output.json');
    fs.writeFileSync(overlay, JSON.stringify({ Replace: replace }));
    const oracleBinary = compiledOracle(sourceDirectory, GO_SOURCE_COMMIT, replace, runProcess);
    const missingFile = path.join(temporary, 'missing-filesystem-operation.json');
    const additionalOperations = [];
    for (;;) {
      capturedInputs.verify();
      fs.writeFileSync(inputFile, JSON.stringify(input));
      fs.rmSync(outputFile, { force: true });
      fs.rmSync(missingFile, { force: true });
      try {
        runProcess(oracleBinary, ['-test.run=^TestAgentTypeOracle$', '-test.count=1'],
          { cwd: sourceDirectory, env: { ...process.env, GOMAXPROCS: '2', AGENT_TYPE_ORACLE_INPUT: inputFile,
            AGENT_TYPE_ORACLE_OUTPUT: outputFile, AGENT_TYPE_ORACLE_FS_REQUEST: missingFile } });
      } catch (error) {
        if (!fs.existsSync(missingFile)) throw error;
        // No result or checker state from this incomplete pass is evidence.
        const missing = JSON.parse(fs.readFileSync(missingFile, 'utf8'));
        capturedInputs.complete(missing);
        additionalOperations.push(missing);
        continue;
      }
      if (fs.existsSync(missingFile)) throw new Error('Compiler hook filesystem protocol: incomplete replay succeeded');
      break;
    }
    capturedInputs.verify();
    const raw = JSON.parse(fs.readFileSync(outputFile, 'utf8'));
    const programFiles = validatedReplayProgram(input, raw.programFiles);
    const nodeLocation = span => {
      const file = span.file.startsWith('bundled:///') ? path.join(VENDORED, path.basename(span.file)) : span.file;
      const captured = input.files[span.file] ?? (span.file.startsWith('bundled:///') ? fs.readFileSync(file, 'utf8') : undefined);
      if (captured === undefined) throw new Error(`Compiler hook returned an uncaptured source: ${span.file}`);
      const source = compilerText(captured);
      const toUtf16 = offset => Buffer.from(source, 'utf8').subarray(0, offset).toString('utf8').length;
      const starts = lineStarts(source);
      return { file: path.relative(input.root, file).split(path.sep).join('/'), start: position(starts, toUtf16(span.start)), end: position(starts, toUtf16(span.end)) };
    };
    const nodeOrigin = span => ({ kind: input.syntaxKind[span.kind], location: nodeLocation(span) });
    const sortedOrigins = spans => spans.map(nodeOrigin).sort(compareOrigins);
    const rawGroups = raw.groups?.map(group => ({ origins: group.origins.map(nodeOrigin), occurrences: group.occurrences.map(nodeLocation) }));
    const nativeRangeGroups = raw.nativeRangeGroups?.map(group => ({ origins: group.origins.map(nodeOrigin).sort(compareOrigins), occurrences: group.occurrences.map(nodeLocation).sort(compareLocations) })).sort(compareGroups);
    const groups = rawGroups?.map(group => ({ origins: [...group.origins].sort(compareOrigins), occurrences: [...group.occurrences].sort(compareLocations) })).sort(compareGroups);
    const callObservations = raw.callObservations?.map(row => ({ location: nodeLocation(row.span), rawCall: row.rawCall, rawTag: row.rawTag, call: row.call }))
      .sort((a, b) => compareLocations(a.location, b.location));
    const inputFiles = Object.keys(input.files).sort(compareExactText);
    // Hash the frozen replay strings, not a later disk read or decoded AST text.
    const inputHashes = inputFiles.map(file => ({ file, sha256: createHash('sha256').update(input.files[file], 'utf8').digest('hex') }));
    return { compiler: '7.0.2', sourceCommit: GO_SOURCE_COMMIT, history, status: 'observed', checkerCount: raw.checkerCount, inputFiles, inputHashes,
      programFiles, nativeProgramFiles: raw.programFiles, apiProgramFiles: input.programFiles,
      fileSystemReplay: { additionalOperations },
      ...(raw.privateNames === undefined ? {} : { privateNames: raw.privateNames }),
      ...(raw.prototypeComparison === undefined ? {} : { prototypeComparison: raw.prototypeComparison }),
      ...(raw.displayEvents === undefined ? {} : { displayEvents: raw.displayEvents, displayAcquisitions: raw.displayAcquisitions }),
      ...(raw.defaultResolutionMode === undefined ? {} : { defaultResolutionMode: raw.defaultResolutionMode,
        selectedModuleResolvedPath: raw.selectedModuleResolvedPath == null ? null : path.relative(input.root, raw.selectedModuleResolvedPath).split(path.sep).join('/'),
        importResolutionModes: raw.importResolutionModes.map(entry => ({...entry,
          resolvedPath: entry.resolvedPath == null ? null : path.relative(input.root, entry.resolvedPath).split(path.sep).join('/')})) }),
      ...(raw.symbolDisplayBytes === undefined ? {} : { symbolDisplayBytes: raw.symbolDisplayBytes,
        symbolDisplayFlags: raw.symbolDisplayFlags, symbolDisplayNodeKind: input.syntaxKind[input.nodeKind], beforeSymbolDisplay: raw.beforeSymbolDisplay }),
      ...(raw.summary === undefined ? {} : { kind: raw.kind, symbolFlags: raw.symbolFlags, symbolMode: raw.symbolMode,
        summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary,
        ...(raw.summaryBytes === undefined ? {} : { summaryBytes: raw.summaryBytes }),
        ...(raw.arrayElement ? { arrayElement: raw.arrayElement } : {}) }),
      ...(raw.rawOrigins ? { rawOrigins: raw.rawOrigins.map(nodeOrigin), combinedOrigins: sortedOrigins(raw.combinedOrigins),
        rawSymbolFlags: raw.rawSymbolFlags, selection: input.selection,
        rawRoots: raw.roots.map(root => root.map(nodeOrigin)), rawSearchRoots: raw.searchRoots.map(root => root.map(nodeOrigin)), rawGroups,
        roots: raw.roots.map(sortedOrigins), searchRoots: raw.searchRoots.map(sortedOrigins), groups, nativeRangeGroups, callObservations,
        prelude: raw.prelude.map(span => ({ kind: input.syntaxKind[span.kind], location: nodeLocation(span) })),
        candidateDemands: raw.candidateDemands.map(nodeLocation),
        candidateLedger: input.candidateLedger,
        candidateObservations: raw.candidateObservations.map(row => ({ location: nodeLocation(row.span), nativeRange: nodeLocation(row.nativeRange), meaning: row.meaning,
          validRaw: row.validRaw, rawTextMatch: row.rawTextMatch, inDomain: row.inDomain, symbolFlags: row.symbolFlags,
          origins: sortedOrigins(row.origins), lexicalOrigins: sortedOrigins(row.lexicalOrigins), relatedOrigins: sortedOrigins(row.relatedOrigins),
          relatedKind: row.relatedKind, ...(row.blocker ? { blocker: row.blocker } : {}) })),
        schedule: { checker: 'one', relationshipBeforeCandidates: history === 'baseline' && raw.searchSetCount > 0,
          searchSetCount: raw.searchSetCount, candidates: raw.candidateDemands.length } } : {}),
      counters: raw.counters, demandFailures: raw.demandFailures, observations: raw.observations, diagnosticCodes: raw.diagnosticCodes,
      compilerOptions: input.compilerOptions };
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    if (process.argv.length !== 3) throw new Error('Usage: node tools/type-oracle.mjs request.json');
    const { config, ...request } = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
    if (typeof config !== 'string' || !request.at) throw new Error('Oracle request requires config and at: {file,line,column}');
    const operation = request.operation ?? 'type';
    if (!['type', 'member'].includes(operation)) throw new Error(`Unknown oracle operation: ${operation}`);
    process.stdout.write(JSON.stringify(await (operation === 'member' ? memberOracle : typeOracle)(config, request), null, 2) + '\n');
  } catch (error) {
    process.stderr.write(`type oracle: ${error.message}\n`);
    process.exitCode = 2;
  }
}

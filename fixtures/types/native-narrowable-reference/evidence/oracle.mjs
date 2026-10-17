// TypeScript 7.0.2 type/member oracle. Each public request owns a fresh checker.
// Implementation is deliberately separate from the lexical reference oracle:
// member identity must not inherit its UnsupportedTarget normalizations.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { loadSemanticApi, lineStarts, position, compareLocations, compareExactText, vendoredLibrary } from "file:///Users/alex/github/typescript-agent-tools/.worktrees/ticket-proposal-typescript-navigation-analysis-and-editing-tools-for-agents-f03075/tools/semantic-oracle.mjs";
import { VENDORED } from "file:///Users/alex/github/typescript-agent-tools/.worktrees/ticket-proposal-typescript-navigation-analysis-and-editing-tools-for-agents-f03075/tools/vendor-typescript-libs.mjs";
import { compilerText } from "file:///Users/alex/github/typescript-agent-tools/.worktrees/ticket-proposal-typescript-navigation-analysis-and-editing-tools-for-agents-f03075/tools/navigation-oracle.mjs";

// Freeze the inputs the compiler actually reads, including config ancestors and
// package manifests. Reading only Program sources changes module resolution.
function captureFileSystem() {
  const files = Object.create(null), realpaths = Object.create(null);
  const realpath = file => {
    try {
      const target = fs.realpathSync(file);
      if (target !== file) realpaths[file] = target;
      return target;
    } catch (error) {
      if (error.code === 'ENOENT' || error.code === 'ENOTDIR') return undefined;
      throw error;
    }
  };
  return { files, realpaths, callbacks: {
    readFile(file) {
      try {
        const text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(fs.readFileSync(file));
        files[file] = text;
        const target = realpath(file);
        if (target === undefined) throw new Error(`Oracle input disappeared while reading ${file}`);
        files[target] = text;
        return compilerText(text);
      } catch (error) {
        if (error.code === 'ENOENT' || error.code === 'ENOTDIR') return null;
        throw error;
      }
    }, realpath,
  } };
}

async function freshRequest(config, action, captureInputs = false) {
  const semantic = await loadSemanticApi();
  const configPath = fs.realpathSync(path.resolve(config));
  const capture = captureInputs ? captureFileSystem() : undefined;
  const api = new semantic.API({ tsserverPath: semantic.binary, cwd: path.dirname(configPath), ...(capture ? { fs: capture.callbacks } : {}) });
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
    return action({ semantic, project, configPath, root: path.dirname(configPath), checker: project.checker,
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
  const indexed = parent.kind === k.LiteralType && parent.parent?.kind === k.IndexedAccessType && parent.parent.indexType === parent;
  const binding = parent.kind === k.BindingElement && parent.propertyName === node;
  if (literal && !declared && !computed && !element && !indexed && !binding) return undefined;
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
  const candidate = memberCandidate(context, syntax.token);
  if (!candidate) throw new Error('UnsupportedTarget: position is not a supported reference name or member anchor');
  return { ...candidate, symbolMode: candidate.syntax === 'shorthandProperty' ? 'shorthand' : candidate.symbolMode };
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
    const calls = history === 'baseline' ? prelude(context, syntax.token) : [];
    const rawSymbol = selected.symbolMode === 'shorthand' ? context.checker.getShorthandAssignmentValueSymbol(syntax.token.parent)
      : context.checker.getSymbolAtLocation(syntax.token);
    if (!rawSymbol || context.checker.isUnknownSymbol(rawSymbol)) throw new Error('UnresolvedTarget: no member symbol');
    const rawOrigins = origins(context, rawSymbol);
    const combined = combinedMember(context, rawSymbol, syntax.token);
    const combinedOrigins = origins(context, combined);
    const candidates = memberCandidates(context, combined.name).map(candidate => {
      const sitePrelude = history === 'baseline' ? prelude(context, candidate.node) : [];
      const symbol = context.checker.getSymbolAtLocation(candidate.node);
      return { location: candidate.location, role: candidate.role, origins: origins(context, symbol), prelude: sitePrelude };
    });
    const referenceGroups = context.checker.getReferencedSymbolsForNode(syntax.token, syntax.offset).map(group => ({
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
const hookFixtures = new URL("file:///Users/alex/github/typescript-agent-tools/.worktrees/ticket-proposal-typescript-navigation-analysis-and-editing-tools-for-agents-f03075/.cc/temp/types-narrowable-reference/");

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
 */
export async function compilerHookOracle(config, request) {
  const operation = request.operation ?? 'type';
  if (!['type', 'member'].includes(operation)) throw new Error(`Unknown oracle operation: ${operation}`);
  const history = request.history ?? 'baseline';
  if (!['baseline', 'cold', 'diagnostics'].includes(history)) throw new Error(`Unknown checking history: ${history}`);
  if (history === 'diagnostics') throw new Error('Unsupported compiler hook history: diagnostics-first checker equivalence remains unproved');
  const sourceDirectory = pinnedGoSource();
  const input = await freshRequest(config, context => {
    const syntax = selectedSyntax(context, request.at, operation === 'member');
    const selected = operation === 'member' ? selectMember(context, syntax) : selectType(context, syntax, request.question);
    const sourceFiles = context.project.program.getSourceFileNames();
    const files = { ...context.capturedInputs.files };
    // Captured inputs retain their original UTF-8 bytes, including a BOM. Only
    // virtual sources absent from readFile need the API's decoded source text.
    for (const file of sourceFiles) if (!Object.hasOwn(files, file)) files[file] = context.project.program.getSourceFile(file).text;
    const candidates = operation === 'member' ? memberCandidates(context, selected.key) : [];
    const byteOffset = (source, offset) => Buffer.byteLength(source.text.slice(0, offset), 'utf8');
    return { files, root: context.root, config: context.configPath, file: syntax.source.fileName,
      offset: byteOffset(syntax.source, syntax.offset), operation,
      question: selected?.kind ?? '', symbolMode: selected?.symbolMode ?? '', demandMode: selected?.demandMode ?? '', normalize: history === 'baseline',
      inspectArrayElement: request.inspectArrayElement === true, narrowMode: request.narrowMode,
      realpaths: context.capturedInputs.realpaths, caseSensitive: context.caseSensitive,
      sourceFiles: sourceFiles.filter(file => !context.project.program.getSourceFileMetadata(file).isDefaultLibrary),
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
  }, true);
  const temporary = fs.mkdtempSync(path.resolve('.cc/temp/type-oracle-hook-'));
  try {
    const replace = {};
    const attach = (source, destination) => { replace[path.join(sourceDirectory, destination)] = source; };
    attach(fileURLToPath(new URL('checker-hook.go', hookFixtures)), 'internal/checker/zz_agent_oracle_hook.go');
    attach(fileURLToPath(new URL('ls-hook.go', hookFixtures)), 'internal/ls/zz_agent_oracle_hook.go');
    attach(fileURLToPath(new URL('oracle-test.go', hookFixtures)), 'internal/ls/zz_agent_oracle_test.go');
    const checkerPath = path.join(sourceDirectory, 'internal/checker/checker.go');
    let checkerSource = fs.readFileSync(checkerPath, 'utf8');
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
    const overlaidChecker = path.join(temporary, 'checker.go');
    fs.writeFileSync(overlaidChecker, checkerSource);
    attach(overlaidChecker, 'internal/checker/checker.go');
    const flowPath = path.join(sourceDirectory, 'internal/checker/flow.go');
    const flowSource = replaceExactly(fs.readFileSync(flowPath, 'utf8'), '\tif f.depth == 2000 {',
      '\tAgentOracleCounter("flowDepth", f.depth)\n\tif f.depth == 2000 {\n\t\tAgentOracleFailure("error_type")');
    const overlaidFlow = path.join(temporary, 'flow.go');
    fs.writeFileSync(overlaidFlow, flowSource);
    attach(overlaidFlow, 'internal/checker/flow.go');
    const relaterPath = path.join(sourceDirectory, 'internal/checker/relater.go');
    let relaterSource = fs.readFileSync(relaterPath, 'utf8');
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
    const overlay = path.join(temporary, 'overlay.json'), inputFile = path.join(temporary, 'input.json'), outputFile = path.join(temporary, 'output.json');
    fs.writeFileSync(overlay, JSON.stringify({ Replace: replace }));
    fs.writeFileSync(inputFile, JSON.stringify(input));
    runProcess('go', ['test', '-p', '2', '-overlay', overlay, './internal/ls', '-run', '^TestAgentTypeOracle$', '-count=1'],
      { cwd: sourceDirectory, env: { ...process.env, GOMAXPROCS: '2', AGENT_TYPE_ORACLE_INPUT: inputFile, AGENT_TYPE_ORACLE_OUTPUT: outputFile } });
    const raw = JSON.parse(fs.readFileSync(outputFile, 'utf8'));
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
    const groups = rawGroups?.map(group => ({ origins: [...group.origins].sort(compareOrigins), occurrences: [...group.occurrences].sort(compareLocations) })).sort(compareGroups);
    const callObservations = raw.callObservations?.map(row => ({ location: nodeLocation(row.span), rawCall: row.rawCall, rawTag: row.rawTag, call: row.call }))
      .sort((a, b) => compareLocations(a.location, b.location));
    const inputFiles = Object.keys(input.files).sort(compareExactText);
    // Hash the frozen replay strings, not a later disk read or decoded AST text.
    const inputHashes = inputFiles.map(file => ({ file, sha256: createHash('sha256').update(input.files[file], 'utf8').digest('hex') }));
    return { compiler: '7.0.2', sourceCommit: GO_SOURCE_COMMIT, history, status: 'observed', inputFiles, inputHashes,
      ...(raw.summary === undefined ? {} : { kind: raw.kind, symbolFlags: raw.symbolFlags, symbolMode: raw.symbolMode,
        summary: raw.summary, errorType: raw.errorType, beforeSummary: raw.beforeSummary, narrowable: raw.narrowable,
        ...(raw.arrayElement ? { arrayElement: raw.arrayElement } : {}) }),
      ...(raw.rawOrigins ? { rawOrigins: raw.rawOrigins.map(nodeOrigin), combinedOrigins: sortedOrigins(raw.combinedOrigins),
        rawSymbolFlags: raw.rawSymbolFlags, selection: input.selection,
        rawRoots: raw.roots.map(root => root.map(nodeOrigin)), rawSearchRoots: raw.searchRoots.map(root => root.map(nodeOrigin)), rawGroups,
        roots: raw.roots.map(sortedOrigins), searchRoots: raw.searchRoots.map(sortedOrigins), groups, callObservations,
        prelude: raw.prelude.map(span => ({ kind: input.syntaxKind[span.kind], location: nodeLocation(span) })),
        candidateDemands: raw.candidateDemands.map(nodeLocation),
        candidateLedger: input.candidateLedger,
        candidateObservations: raw.candidateObservations.map(row => ({ location: nodeLocation(row.span), meaning: row.meaning,
          validRaw: row.validRaw, rawTextMatch: row.rawTextMatch, symbolFlags: row.symbolFlags,
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

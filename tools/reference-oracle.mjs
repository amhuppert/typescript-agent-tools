import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { runParser, parserFailure } from './parser-process.mjs';
import { firstDifference } from './ast-comparison.mjs';

const require = createRequire(import.meta.url);

// The JSON schema and TS6-to-TS7 adjustments are defined against this API.
export function loadCompilerApi(module = process.env.TYPE_SCRIPT_API ?? 'typescript') {
  const ts = require(module);
  if (ts.version !== '6.0.3') throw new Error(`Expected compiler API 6.0.3, received ${ts.version}`);
  return ts;
}

export function nativeCompilerVersion(binary) {
  const result = spawnSync(binary, ['--version'], { encoding: 'utf8', timeout: 10_000 });
  if (result.error || result.signal || result.status !== 0 || !/^Version 7\.\d+\.\d+(?:[-+\w.]*)?\s*$/.test(result.stdout)) {
    throw new Error(`TYPE_SCRIPT_NATIVE must point to a working TypeScript 7 compiler: ${result.error?.message || result.stderr?.trim() || result.stdout?.trim() || `exit ${result.status}, signal ${result.signal}`}`);
  }
  return result.stdout.trim();
}

// TS1540's message, as TypeScript 7.0.2 words it.
const MODULE_KEYWORD = "A 'namespace' declaration should not be declared using the 'module' keyword. Please use the 'namespace' keyword instead.";

// Classify the TS6 tree for TS7 syntax validation. Runners decide whether an
// ineligible fixture is an error or an exclusion; eligible invalid inputs must
// be compared as syntax rejections. Native checks use only strictDiagnostics:
// the errors TypeScript 7 reports from syntax alone that the API and native
// --noCheck leave to the checker. Of those, TS1540 leaves TypeScript 7's tree
// whole, so the Rust CLI must still write the tree (`parsed`) while --check
// rejects the source.
export function classifyReference(ts, reference) {
  let removed;
  const deferred = [], checker = [];
  const containers = new Set([ts.SyntaxKind.SourceFile, ts.SyntaxKind.ModuleBlock, ts.SyntaxKind.ModuleDeclaration]);
  const pending = [[reference, undefined]];
  while (pending.length) {
    const [node, parent] = pending.pop();
    // checkModuleDeclaration: an identifier-named declaration whose keyword is
    // `module`, which the API keeps as the absence of the Namespace flag, at
    // each part of a dotted name, unless checkGrammarModuleElementContext
    // reports the declaration out of place (TS1235) first.
    if (ts.isModuleDeclaration(node) && ts.isIdentifier(node.name)
      && !(node.flags & (ts.NodeFlags.Namespace | ts.NodeFlags.GlobalAugmentation))
      && containers.has(parent?.kind))
      checker.push({ code: 1540, message: MODULE_KEYWORD });
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node) || ts.isImportTypeNode(node))
      && node.attributes?.token === ts.SyntaxKind.AssertKeyword) removed ??= 'assert attributes';
    if (node.kind === ts.SyntaxKind.JSDocUnknownType || node.kind === ts.SyntaxKind.JSDocFunctionType)
      removed ??= 'legacy JSDoc type syntax';
    // The API and native --noCheck leave this grammar error to the checker.
    if (ts.isThrowStatement(node) && ts.isIdentifier(node.expression)
      && !node.expression.escapedText && node.expression.pos === node.expression.end)
      deferred.push({ code: 1142, message: 'Line break not permitted after throw.' });
    ts.forEachChild(node, child => { pending.push([child, node]); });
  }
  const strictDiagnostics = [...deferred, ...checker];
  const diagnostics = reference.parseDiagnostics.filter(d => d.code !== 2458)
    .map(d => ({ code: d.code, message: ts.flattenDiagnosticMessageText(d.messageText, '\n') }));
  const parsed = !diagnostics.length && !deferred.length;
  diagnostics.push(...strictDiagnostics);
  return { eligible: !removed, removed, valid: !diagnostics.length, parsed, diagnostics, strictDiagnostics };
}

// Compare one source text with the compiler API oracle. `name` is the oracle's
// file name; its extension selects TSX and declaration context. When TypeScript
// 7 still has the syntax (eligible), the Rust CLI parses `file`, which must
// keep that extension, or else the source on stdin. A source TypeScript 7
// parses whole has its tree compared, after `normalize` prepares both trees
// for firstDifference, unless under `checkOnly`. Invalid inputs, and every
// input under `checkOnly`, also run with --check.
//
// Returns classifyReference's facts plus `outcome`, the CLI's accepted,
// rejected, or failed (absent when ineligible) for the tree, or else for
// --check, `checkOutcome` when both ran, and `difference`, the first
// disagreement with the oracle, if any: a process failure, acceptance of
// invalid syntax, rejection of valid syntax, or the first differing tree path.
// Differing trees are also returned as `actual` and `expected`. Runners own the
// policy: which facts pass, and what an ineligible or invalid input means.
export function compareSource(ts, binary, { name, source, file, normalize, checkOnly = false, options = {} }) {
  // As TypeScript's getScriptKindFromFileName and ParseOptions::for_file: the
  // extension without case.
  const tsx = /\.tsx$/i.test(name);
  const reference = ts.createSourceFile(name, source, ts.ScriptTarget.Latest, false, tsx ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const facts = classifyReference(ts, reference);
  if (!facts.eligible) return facts;
  const run = args => runParser(binary, [...args, ...(file ? [file] : tsx ? ['--tsx'] : [])],
    file ? options : { ...options, input: source });
  if (facts.parsed && !checkOnly) {
    const result = run([]);
    facts.outcome = result.outcome;
    if (result.outcome !== 'accepted') facts.difference = parserFailure(result);
    else {
      const actual = normalize(JSON.parse(result.stdout), true);
      const expected = normalize(reference, true);
      facts.difference = firstDifference(actual, expected);
      if (facts.difference) Object.assign(facts, { actual, expected });
    }
    if (facts.difference || facts.valid) return facts;
  }
  const result = run(['--check']);
  facts[facts.outcome ? 'checkOutcome' : 'outcome'] = result.outcome;
  if (result.outcome === 'failed') facts.difference = parserFailure(result);
  else if (!facts.valid) {
    if (result.outcome === 'accepted') facts.difference = { error: 'Accepted compiler-invalid syntax', diagnostics: facts.diagnostics };
  } else if (result.outcome === 'rejected') facts.difference = parserFailure(result);
  return facts;
}

// Native exits do not use the Rust CLI's 0/1/2 contract. A nonzero normal exit
// requires file diagnostics; global/setup errors never establish rejection.
export function nativeDiagnostics(result) {
  if (result.error || result.signal || result.status === null)
    throw new Error(`Native process failed: ${result.error?.message || result.signal || 'no exit status'}`);
  const diagnostics = [];
  const output = [result.stdout, result.stderr].filter(Boolean).join('\n');
  for (const match of output.matchAll(/^(?:(.+?)\((\d+),(\d+)\): )?error TS(\d+): ([^\n]*)/gm)) {
    if (!match[1]) throw new Error(`Native compiler setup failed: ${match[0]}`);
    diagnostics.push({ file: path.resolve(match[1]), line: Number(match[2]), column: Number(match[3]), code: Number(match[4]), message: match[5] });
  }
  if (result.status !== 0 && !diagnostics.length)
    throw new Error(`Native compiler failed without file diagnostics: ${output.trim() || `exit ${result.status}`}`);
  return diagnostics;
}

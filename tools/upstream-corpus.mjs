// Test an upstream compiler checkout without adding its fixtures to this repo.
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { normalizer, firstDifference } from './ast-comparison.mjs';
import { walk, units } from './upstream-fixtures.mjs';
const require = createRequire(import.meta.url);
const ts = require(process.env.TYPE_SCRIPT_API ?? 'typescript');
const binary = process.env.TSPARSE ?? path.resolve('target/release/tsparse');
const args = process.argv.slice(2);
const root = args.find(arg => !arg.startsWith('--'));
if (!root) throw new Error('Usage: node tools/upstream-corpus.mjs CHECKOUT [--limit=N] [--match=REGEX] [--check-only] [--positions --flags --scanner-hints]');
const limit = Number(args.find(arg => arg.startsWith('--limit='))?.slice(8) ?? Infinity);
const match = new RegExp(args.find(arg => arg.startsWith('--match='))?.slice(8) ?? '');
const checkOnly = args.includes('--check-only');
const failuresReport = args.find(arg => arg.startsWith('--failures='))?.slice(11);
const selectedUnits = failuresReport ? new Set(JSON.parse(fs.readFileSync(failuresReport, 'utf8')).failures
  .map(test => `${test.fixture}\0${test.name}`)) : undefined;
const reportFile = args.find(arg => arg.startsWith('--report='))?.slice(9) ?? '.cache/upstream-corpus.json';
const normalize = normalizer(args);
const directory = path.resolve('.cache/upstream-units'); fs.mkdirSync(directory, { recursive: true });
function removedSyntax(reference) {
  const pending = [reference];
  while (pending.length) {
    const node = pending.pop();
    if (ts.isModuleDeclaration(node) && !ts.isStringLiteral(node.name)
      && !(node.flags & (ts.NodeFlags.Namespace | ts.NodeFlags.GlobalAugmentation))
      && !(node.flags & ts.NodeFlags.NestedNamespace)) return 'unquoted module';
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node) || ts.isImportTypeNode(node)) && node.attributes?.token === ts.SyntaxKind.AssertKeyword) return 'assert attributes';
    if (node.kind === ts.SyntaxKind.JSDocUnknownType || node.kind === ts.SyntaxKind.JSDocFunctionType) return 'legacy JSDoc type syntax';
    ts.forEachChild(node, child => { pending.push(child); });
  }
}
function strictGrammarDiagnostics(reference) {
  const pending = [reference];
  while (pending.length) {
    const node = pending.pop();
    // The API leaves this ECMAScript grammar error to the checker's TS1142 diagnostic.
    if (ts.isThrowStatement(node) && ts.isIdentifier(node.expression)
      && !node.expression.escapedText && node.expression.pos === node.expression.end)
      return [{ code: 1142, message: 'Line break not permitted after throw.' }];
    ts.forEachChild(node, child => { pending.push(child); });
  }
  return [];
}
const failures = [];
const commit = spawnSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout.trim();
function saveReport(complete) {
  const report = { compilerApi: ts.version, commit, complete, scope: { limit: Number.isFinite(limit) ? limit : null, match: match.source, failuresReport }, count, valid, invalid, removed, passed, bytes, failures };
  fs.writeFileSync(reportFile, JSON.stringify(report, null, 2));
  return report;
}
let count = 0, valid = 0, invalid = 0, passed = 0, bytes = 0, removed = 0;
outer: for (const category of ['conformance', 'compiler']) {
  const folder = path.join(root, 'tests/cases', category);
  if (!fs.existsSync(folder)) continue;
  for (const fixture of walk(folder)) {
    if (!match.test(fixture)) continue;
    for (const { name, source } of units(fixture, fs.readFileSync(fixture, 'utf8'))) {
      if (selectedUnits && !selectedUnits.has(`${path.relative(root, fixture)}\0${name}`)) continue;
      if (count >= limit) break outer;
      const jsx = name.endsWith('.tsx');
      const reference = ts.createSourceFile(name, source, ts.ScriptTarget.Latest, false, jsx ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
      if (removedSyntax(reference)) { removed++; continue; }
      count++; bytes += Buffer.byteLength(source);
      const diagnostics = [...reference.parseDiagnostics.filter(d => d.code !== 2458).map(d => ({ code: d.code, message: ts.flattenDiagnosticMessageText(d.messageText, '\n') })), ...strictGrammarDiagnostics(reference)];
      const referenceValid = !diagnostics.length;
      if (referenceValid) valid++; else invalid++;
      // Keep the extension, including .d.mts/.d.cts, for the Rust ambient context.
      const file = path.join(directory, `${count}-${name.replace(/[^\w.\-]/g, '_')}`);
      fs.writeFileSync(file, source);
      const result = spawnSync(binary, checkOnly || !referenceValid ? ['--check', file] : [file], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, timeout: 10_000 });
      let difference;
      if (result.status === null) difference = { error: 'Process failed or timed out', signal: result.signal };
      else if (!referenceValid && result.status === 0) difference = { error: 'Accepted compiler-invalid syntax', diagnostics };
      else if (referenceValid && result.status !== 0) difference = { error: result.stderr.trim() };
      else if (referenceValid && !checkOnly) difference = firstDifference(normalize(JSON.parse(result.stdout), true), normalize(reference, true));
      if (difference) {
        failures.push({ fixture: path.relative(root, fixture), name, file, ...difference });
        if (failures.length <= 20) console.log(JSON.stringify(failures.at(-1)));
      } else { passed++; fs.unlinkSync(file); }
      if (count % 1000 === 0) {
        saveReport(false);
        console.log(JSON.stringify({ progress: count, valid, invalid, passed, failures: failures.length }));
      }
    }
  }
}
const report = saveReport(true);
console.log(JSON.stringify({ ...report, failures: failures.length }));
process.exitCode = failures.length ? 1 : 0;

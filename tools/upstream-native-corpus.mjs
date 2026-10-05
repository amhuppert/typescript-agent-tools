// Batch native syntax checks while retaining a result for each virtual source file.
import { runParser, parserFailure } from './parser-process.mjs';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { allUnits } from './upstream-fixtures.mjs';
const ts = createRequire(import.meta.url)(process.env.TYPE_SCRIPT_API ?? 'typescript');
const native = process.env.TYPE_SCRIPT_NATIVE;
const binary = process.env.TSPARSE ?? path.resolve('target/release/tsparse');
const root = process.argv.slice(2).find(arg => !arg.startsWith('--'));
if (!root || !native) throw new Error('Usage: TYPE_SCRIPT_NATIVE=... TYPE_SCRIPT_API=... node tools/upstream-native-corpus.mjs CHECKOUT [--limit=N]');
const limit = Number(process.argv.find(a => a.startsWith('--limit='))?.slice(8) ?? Infinity);
const directory = path.resolve('.cache/upstream-native'); fs.mkdirSync(directory, { recursive: true });
const version = spawnSync(native, ['--version'], { encoding: 'utf8' }).stdout.trim();
const commit = spawnSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout.trim();
function strictGrammarError(unit) {
  const reference = ts.createSourceFile(unit.name, unit.source, ts.ScriptTarget.Latest, false, unit.name.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const pending = [reference];
  while (pending.length) {
    const node = pending.pop();
    if (ts.isModuleDeclaration(node) && !ts.isStringLiteral(node.name)
      && !(node.flags & (ts.NodeFlags.Namespace | ts.NodeFlags.GlobalAugmentation | ts.NodeFlags.NestedNamespace))) return 1540;
    if (ts.isThrowStatement(node) && ts.isIdentifier(node.expression)
      && !node.expression.escapedText && node.expression.pos === node.expression.end) return 1142;
    ts.forEachChild(node, child => { pending.push(child); });
  }
}
let count = 0, passed = 0, invalid = 0, strict = 0, bytes = 0;
const failures = [];
function report(complete) {
  fs.writeFileSync('.cache/upstream-native.json', JSON.stringify({ version, commit, complete, count, passed, invalid, strict, bytes, failures }, null, 2));
}
function check(batch) {
  const result = spawnSync(native, ['--ignoreConfig', '--noEmit', '--noCheck', '--noResolve', '--pretty', 'false', '--target', 'esnext', '--module', 'esnext', '--jsx', 'preserve', ...batch.map(test => test.file)],
    { encoding: 'utf8', timeout: 30_000, maxBuffer: 64 * 1024 * 1024 });
  if (result.status === null || result.error) throw new Error(`Native process failed: ${result.error ?? result.signal}`);
  const diagnostics = new Map();
  for (const match of result.stdout.matchAll(/^(.+?)\((\d+),(\d+)\): error TS(\d+): ([^\n]*)/gm)) {
    const key = path.resolve(match[1]);
    const items = diagnostics.get(key) ?? [];
    items.push({ line: Number(match[2]), column: Number(match[3]), code: Number(match[4]), message: match[5] });
    diagnostics.set(key, items);
  }
  if (result.status !== 0 && !diagnostics.size) throw new Error(result.stdout || result.stderr);
  for (const test of batch) {
    const nativeErrors = diagnostics.get(test.file) ?? [];
    const strictError = strictGrammarError(test);
    const valid = !nativeErrors.length && !strictError;
    if (!valid) invalid++;
    if (strictError) strict++;
    const rust = runParser(binary, ['--check', test.file]);
    if (rust.outcome === 'failed' || (rust.outcome === 'accepted') !== valid) {
      const { source, ...identity } = test;
      failures.push({ ...identity, nativeErrors, strictError, rustStatus: rust.status, ...parserFailure(rust) });
      if (failures.length <= 20) console.log(JSON.stringify(failures.at(-1)));
    } else { passed++; fs.unlinkSync(test.file); }
  }
  report(false);
  if (count % 1024 === 0) console.log(JSON.stringify({ count, passed, failures: failures.length }));
}
let batch = [];
for (const unit of allUnits(root)) {
  if (count >= limit) break;
  count++; bytes += Buffer.byteLength(unit.source);
  const file = path.join(directory, `${count}-${unit.name.replace(/[^\w.\-]/g, '_')}`);
  fs.writeFileSync(file, unit.source);
  batch.push({ ...unit, file });
  if (batch.length === 128) { check(batch); batch = []; }
}
if (batch.length) check(batch);
report(true);
console.log(JSON.stringify({ version, count, passed, invalid, strict, bytes, failures: failures.length }));
process.exitCode = failures.length ? 1 : 0;

// Batch native syntax checks while retaining a result for each virtual source file.
import { runParser, parserFailure } from './parser-process.mjs';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { allUnits } from './upstream-fixtures.mjs';
import { loadCompilerApi, nativeCompilerVersion, classifyReference, nativeDiagnostics } from './reference-oracle.mjs';
import { corpusLimit, requireComparisons } from './corpus-selection.mjs';
const ts = loadCompilerApi();
const native = process.env.TYPE_SCRIPT_NATIVE;
const binary = process.env.TSPARSE ?? path.resolve('target/release/tsparse');
const root = process.argv.slice(2).find(arg => !arg.startsWith('--'));
if (!root || !native) throw new Error('Usage: TYPE_SCRIPT_NATIVE=... TYPE_SCRIPT_API=... node tools/upstream-native-corpus.mjs CHECKOUT [--limit=N]');
const limit = corpusLimit(process.argv);
const directory = path.resolve('.cache/upstream-native'); fs.mkdirSync(directory, { recursive: true });
const version = nativeCompilerVersion(native);
const commit = spawnSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout.trim();
let count = 0, passed = 0, invalid = 0, strict = 0, bytes = 0;
const failures = [];
function report(complete) {
  fs.writeFileSync('.cache/upstream-native.json', JSON.stringify({ version, commit, complete, count, passed, invalid, strict, bytes, failures }, null, 2));
}
function check(batch) {
  const result = spawnSync(native, ['--ignoreConfig', '--noEmit', '--noCheck', '--noResolve', '--pretty', 'false', '--target', 'esnext', '--module', 'esnext', '--jsx', 'preserve', ...batch.map(test => test.file)],
    { encoding: 'utf8', timeout: 30_000, maxBuffer: 64 * 1024 * 1024 });
  const diagnostics = new Map();
  const files = new Set(batch.map(test => test.file));
  for (const { file, ...diagnostic } of nativeDiagnostics(result)) {
    if (!files.has(file)) throw new Error(`Unexpected native diagnostic outside the selected batch: ${file}`);
    const items = diagnostics.get(file) ?? [];
    items.push(diagnostic);
    diagnostics.set(file, items);
  }
  for (const test of batch) {
    const nativeErrors = diagnostics.get(test.file) ?? [];
    const reference = ts.createSourceFile(test.name, test.source, ts.ScriptTarget.Latest, false, test.name.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
    const strictError = classifyReference(ts, reference).strictDiagnostics[0]?.code;
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
requireComparisons(count);
report(true);
console.log(JSON.stringify({ version, count, passed, invalid, strict, bytes, failures: failures.length }));
process.exitCode = failures.length ? 1 : 0;

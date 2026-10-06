// Compare eligible TS7 source trees, including syntax rejection checks, without
// changing or copying their files. TS6-only syntax is counted as removed.
import fs from 'node:fs';
import path from 'node:path';
import { COMPARISON_OPTIONS, normalizer, describeComparison } from './ast-comparison.mjs';
import { loadCompilerApi, compareSource } from './reference-oracle.mjs';
import { runnerArguments, requireComparisons } from './corpus-selection.mjs';
const { flags, paths: roots } = runnerArguments(process.argv.slice(2), { options: COMPARISON_OPTIONS, positional: Infinity });
const normalize = normalizer(flags);
if (!roots.length) throw new Error('Usage: node tools/source-corpus.mjs [--positions --flags --scanner-hints] SOURCE_DIRECTORY...');
const ts = loadCompilerApi();
const binary = process.env.TSPARSE ?? path.resolve('target/debug/tsparse');
const ignored = new Set(['.git', '.cache', 'node_modules', 'target', 'vendor', 'dist', 'build']);
function* files(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory() && !entry.name.startsWith('.') && !ignored.has(entry.name)) yield* files(file);
    else if (entry.isFile() && /\.(?:[cm]?ts|tsx)$/.test(entry.name)) yield file;
  }
}
let total = 0, bytes = 0, invalid = 0, removed = 0, passed = 0;
const failures = [];
for (const root of roots) for (const file of files(root)) {
  const source = fs.readFileSync(file, 'utf8');
  const facts = compareSource(ts, binary, { name: file, source, file, normalize, options: { maxBuffer: 128 * 1024 * 1024 } });
  // Syntax removed in TypeScript 7 is excluded; an invalid file passes when rejected.
  if (!facts.eligible) { removed++; continue; }
  if (!facts.valid) invalid++;
  total++; bytes += Buffer.byteLength(source);
  if (facts.difference) {
    failures.push({ file, ...facts.difference });
    if (failures.length <= 20) console.log(JSON.stringify(failures.at(-1)));
  }
  else passed++;
}
requireComparisons(total);
const report = { version: ts.version, compared: describeComparison(normalize.compared), total, bytes, invalid, removed, passed, failures };
fs.mkdirSync('.cache', { recursive: true });
fs.writeFileSync('.cache/source-corpus.json', JSON.stringify(report, null, 2));
console.log(JSON.stringify({ ...report, failures: failures.length }));
process.exitCode = failures.length ? 1 : 0;

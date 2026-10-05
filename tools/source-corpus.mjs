// Compare eligible TS7 source trees, including syntax rejection checks, without
// changing or copying their files. TS6-only syntax is counted as removed.
import { runParser, parserFailure } from './parser-process.mjs';
import fs from 'node:fs';
import path from 'node:path';
import { normalizer, firstDifference } from './ast-comparison.mjs';
import { loadCompilerApi, classifyReference } from './reference-oracle.mjs';
import { requireComparisons } from './corpus-selection.mjs';
const ts = loadCompilerApi();
const binary = process.env.TSPARSE ?? path.resolve('target/debug/tsparse');
const normalize = normalizer(process.argv);
const roots = process.argv.slice(2).filter(arg => !arg.startsWith('--'));
if (!roots.length) throw new Error('Usage: node tools/source-corpus.mjs [--positions --flags --scanner-hints] SOURCE_DIRECTORY...');
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
  const jsx = file.endsWith('.tsx');
  const reference = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, false, jsx ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const classification = classifyReference(ts, reference);
  if (!classification.eligible) { removed++; continue; }
  if (!classification.valid) invalid++;
  total++; bytes += Buffer.byteLength(source);
  const result = runParser(binary, classification.valid ? [file] : ['--check', file], { maxBuffer: 128 * 1024 * 1024 });
  let difference;
  if (result.outcome === 'failed') difference = parserFailure(result);
  else if (!classification.valid && result.outcome === 'accepted') difference = { error: 'Accepted compiler-invalid syntax', diagnostics: classification.diagnostics };
  else if (classification.valid && result.outcome !== 'accepted') difference = parserFailure(result);
  else if (classification.valid) difference = firstDifference(normalize(JSON.parse(result.stdout), true), normalize(reference, true));
  if (difference) {
    failures.push({ file, ...difference });
    if (failures.length <= 20) console.log(JSON.stringify(failures.at(-1)));
  }
  else passed++;
}
requireComparisons(total);
const report = { version: ts.version, total, bytes, invalid, removed, passed, failures };
fs.mkdirSync('.cache', { recursive: true });
fs.writeFileSync('.cache/source-corpus.json', JSON.stringify(report, null, 2));
console.log(JSON.stringify({ ...report, failures: failures.length }));
process.exitCode = failures.length ? 1 : 0;

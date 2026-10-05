// Validate existing source trees without changing or copying their files.
import { runParser, parserFailure } from './parser-process.mjs';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { normalizer, firstDifference } from './ast-comparison.mjs';
const require = createRequire(import.meta.url);
const ts = require(process.env.TYPE_SCRIPT_API ?? 'typescript');
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
let total = 0, bytes = 0, invalid = 0, passed = 0;
const failures = [];
for (const root of roots) for (const file of files(root)) {
  const source = fs.readFileSync(file, 'utf8');
  const jsx = file.endsWith('.tsx');
  const reference = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, false, jsx ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  if (reference.parseDiagnostics.length) { invalid++; continue; }
  total++; bytes += Buffer.byteLength(source);
  const result = runParser(binary, [file], { maxBuffer: 128 * 1024 * 1024 });
  const difference = result.outcome === 'accepted' ? firstDifference(normalize(JSON.parse(result.stdout), true), normalize(reference, true)) : parserFailure(result);
  if (difference) {
    failures.push({ file, ...difference });
    if (failures.length <= 20) console.log(JSON.stringify(failures.at(-1)));
  }
  else passed++;
}
const report = { version: ts.version, total, bytes, invalid, passed, failures };
fs.mkdirSync('.cache', { recursive: true });
fs.writeFileSync('.cache/source-corpus.json', JSON.stringify(report, null, 2));
console.log(JSON.stringify({ ...report, failures: failures.length }));
process.exitCode = failures.length ? 1 : 0;

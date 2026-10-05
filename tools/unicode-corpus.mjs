// Exhaustive escaped identifier checks against the native compiler's scanner.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { loadCompilerApi, nativeCompilerVersion, nativeDiagnostics } from './reference-oracle.mjs';
import { corpusLimit } from './corpus-selection.mjs';
const ts = loadCompilerApi();
const native = process.env.TYPE_SCRIPT_NATIVE;
if (!native) throw new Error('Set TYPE_SCRIPT_NATIVE to the TypeScript 7 compiler.');
const version = nativeCompilerVersion(native);
const limit = corpusLimit(process.argv, 0x110000, 0x110000);
const chunkSize = 4096;
const directory = path.resolve('.cache/unicode'); fs.mkdirSync(directory, { recursive: true });
const file = path.join(directory, 'identifiers.ts');
const globals = path.join(directory, 'globals.d.ts');
fs.writeFileSync(globals, 'interface Array<T> {} interface Boolean {} interface CallableFunction {} interface Function {} interface IArguments {} interface NewableFunction {} interface Number {} interface Object {} interface RegExp {} interface String {}');
const classification = Buffer.alloc(limit);
const differences = [];
for (let start = 0; start < limit; start += chunkSize) {
  const end = Math.min(limit, start + chunkSize);
  const lines = [];
  for (let cp = start; cp < end; cp++) {
    const escape = `\\u{${cp.toString(16)}}`;
    lines.push(`var ${escape} = 0;`, `var a${escape} = 0;`);
  }
  fs.writeFileSync(file, lines.join('\n'));
  const result = spawnSync(native, ['--ignoreConfig', '--noLib', '--noCheck', '--noEmit', '--pretty', 'false', '--target', 'esnext', globals, file],
    { encoding: 'utf8', timeout: 30_000, maxBuffer: 32 * 1024 * 1024 });
  const diagnostics = nativeDiagnostics(result);
  if (diagnostics.some(d => d.file !== file)) throw new Error(`Unexpected native diagnostic outside ${file}`);
  const badLines = new Set(diagnostics.map(d => d.line));
  for (let cp = start; cp < end; cp++) {
    const index = (cp - start) * 2;
    const actual = Number(!badLines.has(index + 1)) | Number(!badLines.has(index + 2)) << 1;
    const expected = Number(ts.isIdentifierStart(cp, ts.ScriptTarget.Latest)) | Number(ts.isIdentifierPart(cp, ts.ScriptTarget.Latest)) << 1;
    classification[cp] = actual;
    if (actual !== expected) differences.push({ codePoint: cp, native: actual, api: expected });
  }
  if (end % 65536 === 0 || end === limit) console.log(JSON.stringify({ checked: end, differences: differences.length }));
}
fs.writeFileSync(path.join(directory, 'classification.bin'), classification);
fs.writeFileSync(path.join(directory, 'report.json'), JSON.stringify({ api: ts.version, native: version, codePoints: limit, differences }, null, 2));
fs.unlinkSync(file);
console.log(JSON.stringify({ api: ts.version, native: version, codePoints: limit, differences: differences.length }));

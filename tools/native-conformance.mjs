import { runParser, parserFailure } from './parser-process.mjs';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { nativeCompilerVersion, nativeDiagnostics } from './reference-oracle.mjs';
import { requireComparisons } from './corpus-selection.mjs';
const native = process.env.TYPE_SCRIPT_NATIVE ?? path.resolve('node_modules/.bin/tsc');
const binary = process.env.TSPARSE ?? path.resolve('target/debug/tsparse');
const version = nativeCompilerVersion(native);
const cases = JSON.parse(fs.readFileSync('fixtures/conformance.json', 'utf8')).map(test => ({ ...test, valid: true }));
cases.push(...JSON.parse(fs.readFileSync('fixtures/invalid.json', 'utf8')).map(test => ({ ...test, valid: false })));
requireComparisons(cases.length);
const directory = path.resolve('.cache/native-conformance'); fs.mkdirSync(directory, { recursive: true });
let failures = 0;
for (const { name, source, jsx, valid, nativeDiagnostic } of cases) {
  const file = path.join(directory, `${name}.${jsx ? 'tsx' : 'ts'}`); fs.writeFileSync(file, source);
  const expected = spawnSync(native, ['--noEmit', ...(nativeDiagnostic ? [] : ['--noCheck']), '--ignoreConfig', '--pretty', 'false', '--target', 'esnext', '--module', 'esnext', '--jsx', 'preserve', file], { encoding: 'utf8', timeout: 10_000 });
  const actual = runParser(binary, ['--check', file]);
  const diagnostics = nativeDiagnostics(expected);
  const nativeValid = nativeDiagnostic ? !diagnostics.some(d => d.file === path.resolve(file) && d.code === nativeDiagnostic) : !diagnostics.length;
  if (nativeValid !== valid || actual.outcome === 'failed' || (actual.outcome === 'accepted') !== valid) {
    console.log(`FAIL ${name}: expected ${valid ? 'valid' : 'invalid'}; native exit ${expected.status}; Rust exit ${actual.status}`);
    console.log(expected.stdout.trim(), actual.outcome === 'accepted' ? '' : JSON.stringify(parserFailure(actual))); failures++;
  }
}
const report = { version, cases: cases.length, passed: cases.length - failures, failures };
fs.writeFileSync('.cache/native-conformance.json', JSON.stringify(report, null, 2));
console.log(JSON.stringify(report)); process.exitCode = failures ? 1 : 0;

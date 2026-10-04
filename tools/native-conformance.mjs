import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
const native = process.env.TYPE_SCRIPT_NATIVE ?? path.resolve('node_modules/.bin/tsc');
const binary = process.env.TSPARSE ?? path.resolve('target/debug/tsparse');
const version = spawnSync(native, ['--version'], { encoding: 'utf8' });
if (version.status !== 0 || !version.stdout.match(/Version 7\./)) throw new Error('TYPE_SCRIPT_NATIVE must point to a working TypeScript 7 compiler');
const cases = JSON.parse(fs.readFileSync('fixtures/conformance.json', 'utf8')).map(test => ({ ...test, valid: true }));
cases.push(...JSON.parse(fs.readFileSync('fixtures/invalid.json', 'utf8')).map(test => ({ ...test, valid: false })));
const directory = path.resolve('.cache/native-conformance'); fs.mkdirSync(directory, { recursive: true });
let failures = 0;
for (const { name, source, jsx, valid, nativeDiagnostic } of cases) {
  const file = path.join(directory, `${name}.${jsx ? 'tsx' : 'ts'}`); fs.writeFileSync(file, source);
  const expected = spawnSync(native, ['--noEmit', ...(nativeDiagnostic ? [] : ['--noCheck']), '--ignoreConfig', '--pretty', 'false', '--target', 'esnext', '--module', 'esnext', '--jsx', 'preserve', file], { encoding: 'utf8', timeout: 10_000 });
  const actual = spawnSync(binary, ['--check', file], { encoding: 'utf8', timeout: 10_000 });
  if (expected.status === null || actual.status === null) throw new Error(`${name}: process failed or timed out`);
  const nativeValid = nativeDiagnostic ? !expected.stdout.includes(`error TS${nativeDiagnostic}:`) : expected.status === 0;
  if (nativeValid !== valid || (actual.status === 0) !== valid) {
    console.log(`FAIL ${name}: expected ${valid ? 'valid' : 'invalid'}; native exit ${expected.status}; Rust exit ${actual.status}`);
    console.log(expected.stdout.trim(), actual.stderr.trim()); failures++;
  }
}
const report = { version: version.stdout.trim(), cases: cases.length, passed: cases.length - failures, failures };
fs.writeFileSync('.cache/native-conformance.json', JSON.stringify(report, null, 2));
console.log(JSON.stringify(report)); process.exitCode = failures ? 1 : 0;

// Deterministic truncations, mutations, and large inputs test bounded failure.
// This is a crash/hang check, not a syntax-acceptance oracle.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const binary = process.env.TSPARSE ?? path.resolve('target/release/tsparse');
const fixtures = JSON.parse(fs.readFileSync('fixtures/conformance.json', 'utf8'));
const limit = Number(process.argv.find(arg => arg.startsWith('--limit='))?.slice(8) ?? Infinity);
let state = 0x5eed1234;
function random(max) {
  state ^= state << 13; state ^= state >>> 17; state ^= state << 5;
  return (state >>> 0) % max;
}
function* cases() {
  const inserts = ['(', ')', '[', ']', '{', '}', '<', '>', ':', ';', '\\', '/*', '"', "'", '`', '?', '\n', '\u0000', '\u2028', '💡'];
  for (const fixture of fixtures) {
    const chars = Array.from(fixture.source);
    const cuts = new Set([0, 1, chars.length - 1]);
    for (let i = 0; i < 12; i++) cuts.add(random(chars.length + 1));
    for (const cut of cuts) {
      if (cut >= 0) yield { name: `${fixture.name}-prefix-${cut}`, jsx: fixture.jsx, source: chars.slice(0, cut).join('') };
    }
    for (let i = 0; i < 24; i++) {
      const at = random(chars.length + 1);
      const length = random(4);
      const insert = inserts[random(inserts.length)];
      yield { name: `${fixture.name}-mutation-${i}`, jsx: fixture.jsx,
        source: [...chars.slice(0, at), insert, ...chars.slice(at + length)].join('') };
    }
  }
  for (const depth of [512, 4096]) {
    for (const [name, source, jsx] of [
      ['parentheses', '('.repeat(depth) + 'x' + ')'.repeat(depth), false],
      ['arrays', '['.repeat(depth) + '0' + ']'.repeat(depth), false],
      ['types', 'type T = ' + 'Promise<'.repeat(depth) + 'T' + '>'.repeat(depth) + ';', false],
      ['blocks', '{'.repeat(depth) + '}'.repeat(depth), false],
      ['jsx', '<X>'.repeat(depth) + '</X>'.repeat(depth), true],
      ['templates', '`a${'.repeat(depth) + 'x' + '}`'.repeat(depth), false],
    ]) yield { name: `${name}-${depth}`, source, jsx };
  }
  yield { name: 'long-operator-chain', source: 'x' + '+x'.repeat(20_000) + ';' };
  yield { name: 'long-member-chain', source: 'x' + '.x'.repeat(20_000) + ';' };
  yield { name: 'unterminated-comment', source: '/*' + 'x'.repeat(1_000_000) };
  yield { name: 'unterminated-string', source: '"' + '\\x'.repeat(100_000) };
  yield { name: 'identifier', source: 'const ' + 'x'.repeat(1_000_000) + ' = 1;' };
}

let count = 0, accepted = 0, rejected = 0;
const failures = [];
const directory = path.resolve('.cache/robustness');
fs.mkdirSync(directory, { recursive: true });
for (const test of cases()) {
  if (count >= limit) break;
  count++;
  const result = spawnSync(binary, ['--check', ...(test.jsx ? ['--tsx'] : [])],
    { input: test.source, encoding: 'utf8', timeout: 3_000, maxBuffer: 1024 * 1024 });
  if (!result.error && result.status === 0) accepted++;
  else if (!result.error && result.status === 1) rejected++;
  else {
    const file = path.join(directory, `${count}-${test.name}.${test.jsx ? 'tsx' : 'ts'}`);
    fs.writeFileSync(file, test.source);
    failures.push({ name: test.name, file, status: result.status, signal: result.signal,
      error: result.error?.message, stderr: result.stderr });
    console.log(JSON.stringify(failures.at(-1)));
  }
}
const report = { seed: '0x5eed1234', count, accepted, rejected, timeoutMs: 3000, failures };
fs.writeFileSync('.cache/robustness-corpus.json', JSON.stringify(report, null, 2));
console.log(JSON.stringify({ ...report, failures: failures.length }));
process.exitCode = failures.length ? 1 : 0;

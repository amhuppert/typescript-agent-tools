import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { createHash } from 'node:crypto';
import { test } from 'node:test';
import { compilerHookOracle } from './type-oracle.mjs';

const union = (key, count) => Array.from({ length: count }, (_, index) => `{ ${key}: ${index} }`).join(' | ');
const product = (left, right) => `declare const root: (${union('left', left)}) & (${union('right', right)});\n`;
const hash = bytes => createHash('sha256').update(bytes).digest('hex');

async function observe(t, source, strict) {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-intersection-limit-')));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const file = path.join(directory, 'cases.ts');
  const config = path.join(directory, 'tsconfig.json');
  fs.writeFileSync(file, source);
  fs.writeFileSync(config, JSON.stringify({ compilerOptions: { strict, noLib: true }, files: ['cases.ts'] }));
  const before = [file, config].map(file => ({ file, sha256: hash(fs.readFileSync(file)) }));
  const answer = await compilerHookOracle(config, {
    at: { file: 'cases.ts', line: 1, column: 15 }, properties: false,
  });
  assert.deepEqual([file, config].map(file => ({ file, sha256: hash(fs.readFileSync(file)) })), before);
  for (const input of before) {
    assert.deepEqual(answer.inputHashes.find(row => row.file === input.file), input);
  }
  return answer;
}

test('native intersection cross-product guard distinguishes just under, at and over 100000', async t => {
  for (const strict of [true, false]) {
    for (const [left, right, size, failed] of [[369, 271, 99999, false], [400, 250, 100000, true], [11, 9091, 100001, true]]) {
      const answer = await observe(t, product(left, right), strict);
      assert.equal(answer.beforeSummary.counters.crossProductUnion, size);
      assert.equal(answer.errorType, failed);
      assert.deepEqual(answer.beforeSummary.demandFailures, failed ? ['union_complexity_limit'] : []);
      assert.deepEqual(answer.beforeSummary.diagnosticCodes.filter(code => code !== 2318), failed ? [2590] : []);
      if (failed) assert.equal(answer.summary, 'any');
    }
  }
});

test('native long flat intersection normalizes to one shallow scalar without the cartesian guard', async t => {
  const source = `declare const root: ${Array.from({ length: 100001 }, () => 'number').join(' & ')};\n`;
  for (const strict of [true, false]) {
    const answer = await observe(t, source, strict);
    assert.equal(answer.summary, 'number');
    assert.equal(answer.errorType, false);
    assert.deepEqual(answer.beforeSummary.demandFailures, []);
    assert.equal(answer.beforeSummary.counters.crossProductUnion ?? 0, 0);
    assert.deepEqual(answer.beforeSummary.diagnosticCodes.filter(code => code !== 2318), []);
  }
});

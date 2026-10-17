import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { compilerHookOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';
import { crossProductSource, crossProductOverflowSource } from '../fixtures/types/compiler-limits/generate.mjs';

async function observe(t, source, strict) {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-template-limit-')));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.writeFileSync(path.join(root, 'cases.ts'), source);
  fs.writeFileSync(path.join(root, 'tsconfig.json'), JSON.stringify({ compilerOptions: { strict, noLib: true }, files: ['cases.ts'] }));
  const offset = source.indexOf('/*#query*/') + '/*#query*/'.length;
  return compilerHookOracle(path.join(root, 'tsconfig.json'), { at: { file: 'cases.ts', ...position(lineStarts(source), offset) } });
}

test('native template product guard distinguishes under, at and over 100000', async t => {
  for (const strict of [true, false]) {
    for (const [left, right, size, failed] of [[3, 33333, 99999, false], [100, 1000, 100000, true], [11, 9091, 100001, true]]) {
      const answer = await observe(t, crossProductSource(left, right), strict);
      assert.equal(answer.beforeSummary.counters.crossProductUnion, size);
      assert.equal(answer.errorType, failed);
      assert.deepEqual(answer.beforeSummary.demandFailures, failed ? ['union_complexity_limit'] : []);
      assert.deepEqual(answer.beforeSummary.diagnosticCodes.filter(code => code !== 2318), failed ? [2590] : []);
      if (failed) assert.equal(answer.summary, 'any');
    }
  }
});

test('native long flat template normalizes before any product limit', async t => {
  const source = 'type /*#query*/Result = `' + '${""}'.repeat(10001) + '`;\n';
  for (const strict of [true, false]) {
    const answer = await observe(t, source, strict);
    assert.equal(answer.summary, '""');
    assert.equal(answer.errorType, false);
    assert.equal(answer.beforeSummary.counters.crossProductUnion ?? 0, 0);
    assert.deepEqual(answer.beforeSummary.demandFailures, []);
    assert.deepEqual(answer.beforeSummary.diagnosticCodes.filter(code => code !== 2318), []);
  }
});

test('native signed product overflow exits before a later Never', async t => {
  // JSON transports the native int through a JavaScript number. The exact
  // native boundary is 2^63-1, whose JSON numeric observation rounds to 2^63.
  const maximum = Number((1n << 63n) - 1n);
  const cases = [
    [62, 'none', 2 ** 62, true],
    [63, 'none', maximum, true],
    [64, 'none', maximum, true],
    [63, 'last', maximum, true],
    [64, 'last', maximum, true],
    [62, 'first', 0, false],
    [63, 'first', 0, false],
    [64, 'first', 0, false],
    [3, 'last', 0, false],
  ];
  for (const strict of [true, false]) {
    for (const [factors, neverPosition, size, failed] of cases) {
      const label = `${strict}/${factors}/${neverPosition}`;
      const answer = await observe(t, crossProductOverflowSource(factors, neverPosition), strict);
      assert.equal(answer.beforeSummary.counters.crossProductUnion, size, label);
      assert.equal(answer.summary, failed ? 'any' : 'never', label);
      assert.equal(answer.errorType, failed, label);
      assert.deepEqual(answer.beforeSummary.demandFailures, failed ? ['union_complexity_limit'] : [], label);
      assert.deepEqual(answer.beforeSummary.diagnosticCodes.filter(code => code !== 2318), failed ? [2590] : [], label);
    }
  }
});

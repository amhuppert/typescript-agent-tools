// Fresh TypeScript 7.0.2 questions pin literal cooking and canonical values.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/literals/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'consumer.ts'), 'utf8');
const expected = JSON.parse(fs.readFileSync(path.join(root, 'expectations.json'), 'utf8'));
const observe = name => {
  const matches = [...source.matchAll(new RegExp('\\b' + name + '\\b', 'g'))];
  assert.equal(matches.length, 1, `unique ${name}`);
  const offset = matches[0].index;
  return typeOracle(path.join(root, 'tsconfig.json'), { at: { file: 'consumer.ts', ...position(lineStarts(source), offset) } });
};

for (const family of ['string', 'number']) {
  test(`fresh alias inspection preserves ${family} literal values from all scanner variants`, async () => {
    for (const row of expected.filter(row => row.base === family && row.value !== 'Infinity')) {
      const answer = await observe(row.name);
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.kind, 'value', row.name);
      assert.equal(answer.summary, row.summary, row.name);
      assert.equal(answer.errorType, false, row.name);
      assert.deepEqual(answer.prelude, [], row.name);
      assert.deepEqual(answer.diagnostics, [], row.name);
    }
  });
}

test('native compiler literal answers retain distinct unpaired UTF-16 units', async () => {
  for (const [name, summary] of [['lone_high', '"\\uD800"'], ['lone_low', '"\\uDC00"']]) {
    const answer = await observe(name);
    assert.equal(answer.summary, summary);
    assert.equal(answer.errorType, false);
  }
});

test('nonfinite numeric literal uses the pinned hook without the public API float transport', async () => {
  const row = expected.find(row => row.value === 'Infinity');
  const offset = source.indexOf(row.name);
  const answer = await compilerHookOracle(path.join(root, 'tsconfig.json'), { operation: 'type', at: { file: 'consumer.ts', ...position(lineStarts(source), offset) } });
  assert.equal(answer.status, 'observed');
  assert.equal(answer.kind, 'value');
  assert.equal(answer.summary, 'Infinity');
  assert.equal(answer.errorType, false);
  // noLib omits ten standard global declarations; the literal operation does
  // not consume those missing globals. They are distinct from demand recovery.
  assert.deepEqual(answer.diagnosticCodes, Array(10).fill(2318));
  assert.deepEqual(answer.demandFailures, []);
});

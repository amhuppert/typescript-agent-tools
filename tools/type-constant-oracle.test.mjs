import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/constants/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'consumer.ts'), 'utf8');
const expected = JSON.parse(fs.readFileSync(path.join(root, 'expectations.json'), 'utf8'));

for (const family of ['literal', 'primitive']) {
  test(`fresh compiler constant expressions establish ${family} answers`, async () => {
    for (const row of expected.filter(row => row.kind === family)) {
      const offset = [...source.matchAll(new RegExp('\\b' + row.name + '\\b', 'g'))];
      assert.equal(offset.length, 1);
      const answer = await typeOracle(path.join(root, 'tsconfig.json'), {
        at: { file: 'consumer.ts', ...position(lineStarts(source), offset[0].index) },
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.kind, 'value', row.name);
      assert.equal(answer.summary, row.summary, row.name);
      assert.equal(answer.errorType, false, row.name);
      assert.deepEqual(answer.prelude, [], row.name);
    }
  });
}

test('non-strict truthiness retains the compiler nullability facts for every literal class', async () => {
  for (const row of expected.filter(row => row.feature === 'truthiness')) {
    const offset = source.indexOf(row.name);
    const answer = await typeOracle(path.join(root, 'tsconfig-nonstrict.json'), {
      at: { file: 'consumer.ts', ...position(lineStarts(source), offset) },
    });
    assert.equal(answer.summary, row.nonStrictSummary, row.name);
    assert.equal(answer.errorType, false, row.name);
  }
});

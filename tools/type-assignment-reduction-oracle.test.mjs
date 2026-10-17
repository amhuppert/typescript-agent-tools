import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/assignment-reduction/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'flow.ts'), 'utf8');
const at = marker => {
  const label = `/*${marker}*/ `;
  const start = source.indexOf(label);
  assert.ok(start >= 0, marker);
  return { file: 'flow.ts', ...position(lineStarts(source), start + label.length) };
};

test('native scalar and fresh Boolean assignment reductions retain the completed declaration identity', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [marker, summary] of [
      ['scalar-join', 'string | number'], ['boolean-join', 'boolean'],
      ['union-assigned', 'string | number'], ['wrong-assigned', 'string | number'],
      ['missing-assigned', 'string | number'], ['unused-assigned', 'number'],
    ]) {
      const answer = await typeOracle(path.join(directory, config), { at: at(marker), properties: false });
      assert.equal(answer.kind, 'expression', marker);
      assert.equal(answer.summary, summary, `${config}: ${marker}`);
      assert.equal(answer.errorType, false, marker);
      assert.deepEqual(answer.prelude, []);
    }
  }
});

test('a union assignment demands the missing RHS while a nonunion assignment skips it', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [marker, hasMissing] of [['missing-assigned', true], ['unused-assigned', false]]) {
      const answer = await compilerHookOracle(path.join(directory, config), { operation: 'type', at: at(marker) });
      assert.deepEqual(answer.beforeSummary.demandFailures, []);
      assert.equal(answer.beforeSummary.diagnosticCodes.includes(2304), hasMissing, `${config}: ${marker}`);
    }
  }
});

// Own property-name questions have the same prelude even for UTF-16-only names.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-pending-values/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'values.ts'), 'utf8');
function offset(marker) {
  const text = `/*#${marker}*/`;
  const start = source.indexOf(text);
  assert.notEqual(start, -1, marker);
  assert.equal(source.indexOf(text, start + text.length), -1);
  return start + text.length;
}
async function observe(marker) {
  const answer = await typeOracle(path.join(root, 'tsconfig.json'), {
    at: { file: 'values.ts', ...position(lineStarts(source), offset(marker)) }, properties: false,
  });
  assert.equal(answer.compiler, '7.0.2');
  assert.equal(answer.history, 'baseline');
  assert.equal(answer.kind, 'value');
  assert.equal(answer.summary, 'number');
  assert.equal(answer.errorType, false);
  assert.deepEqual(answer.diagnostics, []);
  assert.deepEqual(answer.location.start, position(lineStarts(source), offset(marker)));
  assert.deepEqual(answer.origins.map(origin => origin.kind), ['PropertyAssignment']);
  return answer;
}

test('direct unpaired property names remain symbol values outside calls', async () => {
  for (const marker of ['outsideHigh', 'outsideLow']) {
    const answer = await observe(marker);
    assert.deepEqual(answer.prelude, []);
  }
});

test('every direct name spelling retains its actual enclosing-call prelude', async () => {
  for (const marker of ['insideHigh', 'insideLow', 'insidePair', 'insideCooked', 'insideNumeric', 'insideIdentifier']) {
    const answer = await observe(marker);
    assert.deepEqual(answer.prelude.map(row => row.kind), ['CallExpression']);
    const line = source.lastIndexOf('\n', offset(marker)) + 1;
    assert.deepEqual(answer.prelude[0].location.start, position(lineStarts(source), line));
  }
});

// Startup's absent Object alone proves nothing. This observation is emitted
// only in getPropertyOfType's actual global Object augmentation branch.
for (const config of ['tsconfig', 'object']) {
  test(`exact property spelling and consumed Object augmentation (${config})`, async () => {
    const rows = [];
    for (const marker of ['outsideHigh', 'outsideLow', 'insideHigh', 'insideLow',
      'insidePair', 'insideCooked', 'insideNumeric', 'insideIdentifier']) {
      const answer = await compilerHookOracle(path.join(root, `${config}.json`), {
        at: {file: 'values.ts', ...position(lineStarts(source), offset(marker))},
      });
      assert.equal(answer.compiler, '7.0.2');
      rows.push({name: marker, summary: answer.summary, errorType: answer.errorType,
        demandFailures: answer.beforeSummary.demandFailures,
        codes: answer.beforeSummary.diagnosticCodes,
        augmentation: Object.fromEntries(Object.entries(answer.beforeSummary.observations)
          .filter(([name]) => name.startsWith('objectAugmentation:')))});
    }
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-pending-values-${config}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${config}.json`), 'utf8')));
  });
}

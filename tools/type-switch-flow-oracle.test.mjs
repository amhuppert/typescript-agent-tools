import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-switch-flow/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'flow.ts'), 'utf8');
const observations = JSON.parse(fs.readFileSync(path.join(directory, 'native-observations.json'), 'utf8'));
const markers = new Map([...source.matchAll(/\/\*#([^*]+)\*\/ /g)].map(match => [
  match[1], match.index + match[0].length,
]));
assert.equal(markers.size, 43);
assert.equal(observations.length, 86);

async function observedQuestions(predicate) {
  const selected = observations.filter(predicate);
  assert.ok(selected.length > 0);
  for (const expected of selected) {
    const offset = markers.get(expected.marker);
    assert.notEqual(offset, undefined, expected.marker);
    const answer = await typeOracle(path.join(directory, expected.config), {
      at: { file: 'flow.ts', ...position(lineStarts(source), offset) }, properties: false,
    });
    assert.equal(answer.compiler, '7.0.2');
    assert.equal(answer.history, 'baseline');
    assert.equal(answer.kind, expected.kind, `${expected.config}: ${expected.marker}`);
    assert.equal(answer.summary, expected.summary, `${expected.config}: ${expected.marker}`);
    assert.equal(answer.errorType, expected.errorType, `${expected.config}: ${expected.marker}`);
    assert.deepEqual(answer.prelude, [], `${expected.config}: ${expected.marker}`);
  }
}

test('native Switch preserves direct, unrelated, grouped, default and fallthrough questions', async () => {
  await observedQuestions(row => !/function$|witness|nil-|duplicate|host-/.test(row.marker));
});

test('native typeof Switch witnesses preserve nil, duplicate, escaped, template and UTF16 distinctions', async () => {
  await observedQuestions(row => !/function$/.test(row.marker) && /witness|nil-|duplicate|host-/.test(row.marker));
});

test('native Switch exhaustiveness controls actual inferred return types in both null-check states', async () => {
  await observedQuestions(row => /function$/.test(row.marker));
});

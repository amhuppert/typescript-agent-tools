// Fresh root demands and separate property demands pin lazy declaration behavior.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-lazy-types/', import.meta.url));
function at(file, marker, token) {
  const text = fs.readFileSync(path.join(root, file), 'utf8');
  const start = text.indexOf(marker);
  assert.notEqual(start, -1, marker);
  assert.equal(text.indexOf(marker, start + marker.length), -1, `unique ${marker}`);
  return { file, ...position(lineStarts(text), start + marker.indexOf(token)) };
}
async function observe(marker, question, summary, properties) {
  const result = await typeOracle(path.join(root, 'tsconfig.json'), {
    at: at('consumer.ts', marker, marker), properties,
  });
  assert.equal(result.compiler, '7.0.2');
  assert.equal(result.history, 'baseline');
  assert.equal(result.kind, question);
  assert.equal(result.summary, summary);
  assert.equal(result.errorType, false);
  assert.deepEqual(result.prelude, []);
  assert.deepEqual(result.diagnostics, []);
  return result;
}

test('named declared/value roots do not demand valid conditional or unresolved member types', async () => {
  for (const [marker, question, summary] of [
    ['ImportedLazy', 'declared', 'Lazy'], ['importedValue', 'value', 'Lazy'],
    ['ImportedBroken', 'declared', 'Broken'], ['importedBrokenValue', 'value', 'Broken'],
  ]) {
    const result = await observe(marker, question, summary, false);
    assert.deepEqual(result.properties, []);
  }
});

test('separate member demands distinguish ordinary conditional result from consumed recovery', async () => {
  const lazy = await observe('ImportedLazy', 'declared', 'Lazy', true);
  assert.deepEqual(lazy.properties.map(property => [property.name, property.summary, property.errorType]),
    [['known', 'string', false], ['deferred', 'number', false]]);
  const broken = await observe('ImportedBroken', 'declared', 'Broken', true);
  assert.deepEqual(broken.properties.map(property => [property.name, property.summary, property.errorType]),
    [['known', 'number', false], ['missing', 'MissingType', true]]);
  for (const result of [lazy, broken]) {
    assert.deepEqual(result.properties.map(property => property.origins.map(origin => origin.kind)),
      [['PropertySignature'], ['PropertySignature']]);
  }
});


test('repeated alias member demands retain one valid canonical declared type', async () => {
  const rootOnly = await observe('ImportedRepeated', 'declared', 'Repeated', false);
  assert.deepEqual(rootOnly.properties, []);
  const expanded = await observe('ImportedRepeated', 'declared', 'Repeated', true);
  assert.deepEqual(expanded.properties.map(property => [property.name, property.summary, property.errorType]),
    [['first', 'number', false], ['second', 'number', false]]);
});

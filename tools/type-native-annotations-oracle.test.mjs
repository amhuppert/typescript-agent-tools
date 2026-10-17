// Relevant exact leaf-operation controls for the native annotation increment.
// Whole-program diagnostic history is deliberately a separate observation.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-annotations/', import.meta.url));
const config = path.join(directory, 'tsconfig.json');
function at(file, marker, within = marker) {
  const text = fs.readFileSync(path.join(directory, file), 'utf8');
  const start = text.indexOf(marker);
  assert.notEqual(start, -1, marker);
  assert.equal(text.indexOf(marker, start + marker.length), -1, `unique marker ${marker}`);
  return { file, ...position(lineStarts(text), start + marker.indexOf(within)) };
}
async function expect(file, marker, within, kind, summary, extra = {}) {
  const result = await typeOracle(config, { at: at(file, marker, within), ...extra });
  assert.equal(result.compiler, '7.0.2');
  assert.equal(result.history, 'baseline');
  assert.equal(result.kind, kind);
  assert.equal(result.summary, summary);
  assert.equal(result.errorType, false);
  assert.deepEqual(result.prelude, []);
  return result;
}

test('primitive annotations and cross-file type-reference aliases expose value, declared and type questions', async () => {
  await expect('consumer.ts', 'annotated,', 'annotated', 'value', 'string');
  await expect('consumer.ts', 'numberValue,', 'numberValue', 'value', 'number');
  await expect('consumer.ts', 'Renamed }', 'Renamed', 'declared', 'string');
  await expect('model.ts', 'annotated: Label', 'annotated', 'value', 'string');
  await expect('model.ts', 'annotated: Label', 'Label', 'type', 'string');
  await expect('model.ts', 'ownPrimitive: string', 'string', 'type', 'string');
  await expect('types.ts', 'Label = string', 'Label', 'declared', 'string');
  await expect('types.ts', 'Renamed = Label', 'Renamed', 'declared', 'string');
});

test('declared variable type demand does not check an unrelated initializer or require one', async () => {
  for (const name of ['mismatch', 'uninitialized', 'missingInitializer']) {
    const result = await expect('consumer.ts', name, name, 'value', 'string');
    assert.deepEqual(result.diagnostics, [], 'fresh type demand has no whole-program diagnostic pass');
  }
  const diagnosed = await expect('consumer.ts', 'mismatch', 'mismatch', 'value', 'string', { diagnostics: true });
  assert.ok(diagnosed.diagnostics.some(row => row.code === 2322));
  assert.ok(diagnosed.diagnostics.some(row => row.code === 2304));
});

test('literal initializer token is an expression question separate from its variable declaration', async () => {
  await expect('model.ts', 'ownPrimitive: string = "ready"', '"ready"', 'expression', '"ready"');
  await expect('model.ts', 'numberValue: Count = 1', '1', 'expression', '1');
});

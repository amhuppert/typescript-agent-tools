// Relevant pinned expectations for the first native cross-file increment.
// This compiler baseline is not evidence that the Rust checker implements it.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-vertical/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'consumer.ts'), 'utf8');
const at = name => {
  const start = source.indexOf(name);
  assert.notEqual(start, -1, name);
  assert.equal(source.indexOf(name, start + name.length), -1, `unique ${name}`);
  return { file: 'consumer.ts', ...position(lineStarts(source), start) };
};
const origin = (file, name) => {
  const text = fs.readFileSync(path.join(directory, file), 'utf8');
  const start = text.indexOf(name);
  assert.notEqual(start, -1, name);
  return { kind: 'PropertyAssignment', location: { file,
    start: position(lineStarts(text), start), end: position(lineStarts(text), start + name.length) } };
};
const observe = name => typeOracle(path.join(directory, 'tsconfig.json'), { at: at(name) });

async function object(name, file) {
  const answer = await observe(name);
  assert.equal(answer.compiler, '7.0.2');
  assert.equal(answer.history, 'baseline');
  assert.equal(answer.kind, 'value');
  assert.equal(answer.summary, '{ label: string; count: number; }');
  assert.equal(answer.errorType, false);
  assert.deepEqual(answer.prelude, []);
  assert.deepEqual(answer.diagnostics, []);
  assert.deepEqual(answer.properties.map(row => [row.name, row.summary, row.errorType]),
    [['label', 'string', false], ['count', 'number', false]]);
  assert.deepEqual(answer.properties.map(row => row.origins),
    [[origin(file, 'label')], [origin(file, 'count')]]);
  return answer;
}

test('direct and renamed barrel aliases infer the same ordinary object and own property origins', async () => {
  const direct = await object('direct', 'model.ts');
  const renamed = await object('renamed', 'model.ts');
  assert.deepEqual(direct.origins, renamed.origins);
  assert.equal(direct.origins.length, 1);
  assert.equal(direct.origins[0].kind, 'VariableDeclaration');
  assert.equal(direct.origins[0].location.file, 'model.ts');
  assert.equal(direct.compilerOptions.strict, true);
});

test('type-only alias inspection of a value-only variable still exposes its value type', async () => {
  await object('typeOnly', 'model.ts');
});

test('const primitive and mutable primitive preserve distinct literal widening', async () => {
  const constant = await observe('text');
  assert.equal(constant.kind, 'value');
  assert.equal(constant.summary, '"ready"');
  assert.equal(constant.errorType, false);
  const mutable = await observe('mutable');
  assert.equal(mutable.kind, 'value');
  assert.equal(mutable.summary, 'string');
  assert.equal(mutable.errorType, false);
});

test('external dependency object members preserve origins outside the searched inputs', async () => {
  await object('remote', 'node_modules/native-fixture/index.ts');
});

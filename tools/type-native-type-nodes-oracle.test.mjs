// Fresh native controls for the declared literal/plain structured type family.
// Properties are observed through the pinned checker, not synthesized here.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-type-nodes/', import.meta.url));
function at(file, marker, token = marker) {
  const text = fs.readFileSync(path.join(root, file), 'utf8');
  const start = text.indexOf(marker);
  assert.notEqual(start, -1, marker);
  assert.equal(text.indexOf(marker, start + marker.length), -1, `unique ${marker}`);
  return { file, ...position(lineStarts(text), start + marker.indexOf(token)) };
}
async function observe(file, marker, token, kind, summary) {
  const result = await typeOracle(path.join(root, 'tsconfig.json'), { at: at(file, marker, token) });
  assert.equal(result.compiler, '7.0.2');
  assert.equal(result.history, 'baseline');
  assert.equal(result.kind, kind);
  assert.equal(result.summary, summary);
  assert.equal(result.errorType, false);
  assert.deepEqual(result.prelude, []);
  assert.deepEqual(result.diagnostics, []);
  return result;
}

test('literal type declarations, imports, annotations and literal tokens keep literal values', async () => {
  for (const [name, summary] of [['Word', '"ready"'], ['Digit', '7']]) {
    await observe('types.ts', `type ${name} =`, name, 'declared', summary);
    await observe('consumer.ts', `Imported${name}`, `Imported${name}`, 'declared', summary);
  }
  await observe('consumer.ts', 'importedLiteral', 'importedLiteral', 'value', '"ready"');
  await observe('consumer.ts', 'importedNumber', 'importedNumber', 'value', '7');
  await observe('types.ts', 'Word = "ready"', '"ready"', 'type', '"ready"');
  await observe('types.ts', 'Digit = 7', '7', 'type', '7');
});

test('plain type literals preserve member order, readonly declarations and source origins', async () => {
  const answer = await observe('consumer.ts', 'importedInline', 'importedInline', 'value', '{ readonly title: string; count: number; }');
  assert.deepEqual(answer.properties.map(row => [row.name, row.summary]), [['title', 'string'], ['count', 'number']]);
  assert.deepEqual(answer.properties.map(row => row.origins.map(origin => [origin.kind, origin.location.file, origin.location.start.line])),
    [[['PropertySignature', 'types.ts', 5]], [['PropertySignature', 'types.ts', 5]]]);
});

test('object alias spelling differs from imported labels and interface spelling remains nominal', async () => {
  for (const [marker, question, summary] of [
    ['ImportedShape', 'declared', 'RecordShape'], ['ImportedFace', 'declared', 'Face'],
    ['importedRecord', 'value', 'RecordShape'], ['importedObject', 'value', 'Face'],
  ]) {
    const answer = await observe('consumer.ts', marker, marker, question, summary);
    assert.deepEqual(answer.properties.map(row => [row.name, row.summary]), [['title', 'string'], ['count', 'number']]);
  }
  await observe('types.ts', 'type RecordShape =', 'RecordShape', 'declared', 'RecordShape');
  await observe('types.ts', 'interface Face', 'Face', 'declared', 'Face');
});

test('ordinary property-signature declaration names ask value questions with distinct own roots', async () => {
  for (const [marker, summary, line] of [
    ['Face { readonly title', 'string', 4], ['OtherFace { title', 'number', 10],
    ['RecordShape = { readonly title', 'string', 3],
    ['EscapedFace { \\u0074itle', 'string', 11],
  ]) {
    const token = marker.includes('\\u0074') ? '\\u0074itle' : 'title';
    const answer = await observe('types.ts', marker, token, 'value', summary);
    assert.equal(answer.origins.length, 1);
    assert.equal(answer.origins[0].kind, 'PropertySignature');
    assert.equal(answer.origins[0].location.file, 'types.ts');
    assert.equal(answer.origins[0].location.start.line, line);
  }
});

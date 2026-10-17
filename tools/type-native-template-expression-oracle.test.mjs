import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle, compilerHookOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-template-expressions/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const expected = JSON.parse(fs.readFileSync(path.join(root, 'expected.json'), 'utf8'));
const markers = [...source.matchAll(/\/\*#([^*]+)\*\//g)].map(match => match[1]);
assert.deepEqual(markers, Object.keys(expected));
for (const config of ['tsconfig.json', 'exact.json', 'loose.json']) {
 test(`native template expressions (${config})`, async () => {
  for (const marker of markers) {
   const tag = `/*#${marker}*/`;
   const at = {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length)};
   const api = await typeOracle(path.join(root, config), {at, properties: false});
   const native = await compilerHookOracle(path.join(root, config), {operation: 'type', at});
   assert.equal(api.compiler, '7.0.2');
   const wanted = expected[marker];
   assert.deepEqual([api.kind, api.summary, api.errorType],
    [wanted.kind, wanted.summary, wanted.errorType], marker);
   assert.deepEqual([native.summary, native.errorType], [wanted.summary, wanted.errorType], marker);
   assert.deepEqual(native.beforeSummary.diagnosticCodes, wanted.codes, marker);
   assert.deepEqual(native.beforeSummary.demandFailures, [], marker);
  }
 });
}

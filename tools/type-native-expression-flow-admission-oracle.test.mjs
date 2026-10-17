import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-expression-operators/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const profile of ['tsconfig', 'loose', 'exact']) {
  test(`native nested conditional read changes its declared boolean (${profile})`, async () => {
    const needle = 'booleanValue ? 1 : 2';
    assert.equal(source.split(needle).length, 2);
    const at = {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(needle))};
    const native = await compilerHookOracle(path.join(root, `${profile}.json`), {at});
    const api = await typeOracle(path.join(root, `${profile}.json`), {at, properties: false});
    assert.equal(native.compiler, '7.0.2');
    assert.deepEqual([native.summary, native.errorType], ['true', false]);
    assert.deepEqual([api.summary, api.errorType], ['true', false]);
  });
}

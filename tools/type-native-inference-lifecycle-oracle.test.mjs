import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-inference-lifecycle/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
test('native later relations remain ordinary after a consumed narrowed initializer', async () => {
  const offset = source.indexOf('recoveryEnvelope:');
  assert.ok(offset >= 0);
  const answer = await typeOracle(path.join(root, 'tsconfig.json'),
    {at:{file:'cases.ts',...position(lineStarts(source),offset)}, properties:true});
  assert.equal(answer.compiler, '7.0.2');
  assert.equal(answer.errorType, false);
  assert.deepEqual(answer.properties.map(property => [property.name,property.summary,property.errorType]),
    [['failed','"chosen"',false],['healthy','"chosen"',false],['repeated','"chosen"',false]]);
});

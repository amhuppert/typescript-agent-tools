import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-nominal-relations/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'sites.ts'), 'utf8');
const cases = [
  [
    "protectedInherited",
    "chosen"
  ],
  [
    "protectedOverride",
    "chosen"
  ],
  [
    "protectedPublicOverride",
    "chosen"
  ],
  [
    "protectedTransitive",
    "chosen"
  ],
  [
    "protectedUnrelated",
    "same"
  ],
  [
    "protectedStructural",
    "same"
  ],
  [
    "protectedToPublic",
    "same"
  ],
  [
    "privateInherited",
    "chosen"
  ],
  [
    "privateSeparate",
    "same"
  ],
  [
    "privateToPublic",
    "same"
  ],
  [
    "publicToPrivate",
    "same"
  ],
  [
    "privateGenericSame",
    "chosen"
  ],
  [
    "privateGenericDifferent",
    "same"
  ],
  [
    "protectedGeneric",
    "chosen"
  ],
  [
    "nominalSkippedSource",
    "same"
  ],
  [
    "nominalSkippedTarget",
    "same"
  ],
  [
    "nominalConsumedSource",
    "same"
  ]
];
test('native private and protected comparison preserves declaring classes and demand order', async () => {
  for (const [name, expected] of cases) {
    const offset = source.indexOf(`${name}Result =`);
    assert.ok(offset >= 0, name);
    const at = {file:'sites.ts', ...position(lineStarts(source), offset)};
    const config = path.join(root, 'tsconfig.json');
    const answer = await typeOracle(config, {at, properties:false});
    assert.equal(answer.compiler, '7.0.2', name);
    assert.equal(answer.summary, JSON.stringify(expected), name);
    assert.equal(answer.errorType, false, name);
    const observed = await compilerHookOracle(config, {operation:'type', at});
    assert.equal(observed.summary, answer.summary, name);
    assert.equal(observed.errorType, false, name);
    assert.deepEqual(observed.beforeSummary.demandFailures, [], name);
    assert.equal(observed.beforeSummary.diagnosticCodes.filter(code => code === 2304 || code === 2552).length,
      name === 'nominalConsumedSource' ? 1 : 0, name);
    assert.ok(!observed.beforeSummary.diagnosticCodes.includes(2345), name);
  }
});

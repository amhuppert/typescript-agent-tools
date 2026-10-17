import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-inference-sessions/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'sites.ts'), 'utf8');
const cases = [
  ['parameterPriority', 'chosen', false],
  ['returnCombination', 'chosen', false],
  ['returnConstraintFilter', 'chosen', false],
  ['constraintReject', 'same', false],
  ['dependentDefault', 'same', false],
  ['forwardDefault', 'chosen', false],
  ['unusedDefault', 'chosen', false],
  ['neededDefault', 'chosen', true],
  ['missingConstraint', 'chosen', true],
  ['primitiveLiteral', 'chosen', false],
  ['constLiteral', 'chosen', false],
  ['nestedObject', 'chosen', false],
  ['referenceArguments', 'chosen', false],
  ['callbackInference', 'chosen', false],
  ['candidateConflict', 'same', false],
  ['unionMatching', 'chosen', false],
  ['recursiveConstraint', 'chosen', false],
];

for (const profile of ['tsconfig.json', 'loose.json']) {
  test(`native contextual inference preserves candidates, defaults and nested traversal: ${profile}`, async () => {
    for (const [name, selected, missing] of cases) {
      const offset = source.indexOf(`${name}Result =`);
      assert.ok(offset >= 0, name);
      const at = { file: 'sites.ts', ...position(lineStarts(source), offset) };
      const config = path.join(directory, profile);
      const answer = await typeOracle(config, { at, properties: false });
      assert.equal(answer.kind, 'value', name);
      assert.equal(answer.summary, `"${selected}"`, name);
      assert.equal(answer.errorType, false, name);
      assert.deepEqual(answer.prelude, [], name);
      assert.deepEqual(answer.diagnostics, [], name);
      const observed = await compilerHookOracle(config, { operation: 'type', at });
      assert.equal(observed.summary, answer.summary, name);
      assert.equal(observed.errorType, false, name);
      assert.deepEqual(observed.beforeSummary.demandFailures, [], name);
      assert.equal(observed.beforeSummary.diagnosticCodes.filter(code => code === 2304 || code === 2552).length,
        missing ? 1 : 0, `${name}: original missing dependency ${JSON.stringify(observed.beforeSummary.diagnosticCodes)}`);
      assert.ok(!observed.beforeSummary.diagnosticCodes.includes(2345), name);
    }
  });
}

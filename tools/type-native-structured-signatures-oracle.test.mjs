import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-structured-signatures/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'sites.ts'), 'utf8');

async function compare(name, summary, missingVisited, profile) {
  const offset = source.indexOf(`${name}Result =`);
  assert.ok(offset >= 0, name);
  const at = { file: 'sites.ts', ...position(lineStarts(source), offset) };
  const config = path.join(directory, profile);
  const label = `${profile}:${name}`;
  // Fresh public output and same-checker demand observations answer distinct
  // questions. Neither pre-checks unrelated erroneous annotations.
  const publicAnswer = await typeOracle(config, { at, properties: false });
  assert.equal(publicAnswer.kind, 'value', label);
  assert.equal(publicAnswer.summary, `"${summary}"`, label);
  assert.equal(publicAnswer.errorType, false, label);
  assert.deepEqual(publicAnswer.prelude, [], label);
  assert.deepEqual(publicAnswer.diagnostics, [], label);
  const answer = await compilerHookOracle(config, { operation: 'type', at });
  assert.equal(answer.kind, 'value', label);
  assert.equal(answer.summary, publicAnswer.summary, label);
  assert.equal(answer.errorType, false, label);
  assert.deepEqual(answer.beforeSummary.demandFailures, [], label);
  assert.equal(answer.beforeSummary.diagnosticCodes.filter(code => code === 2304 || code === 2552).length,
    missingVisited ? 1 : 0, `${label}: actual missing child demand ${JSON.stringify(answer.beforeSummary.diagnosticCodes)}`);
  assert.ok(!answer.beforeSummary.diagnosticCodes.includes(2345),
    `${label}: a selected overload avoids call-rejection recovery`);
}

const groups = [
  ['signature returns preserve target-first reads and Void/Any skipping', [
    ['returnVoid', 'chosen', false], ['returnAny', 'chosen', false],
    ['returnUnknown', 'chosen', true], ['returnNumber', 'same', true],
    ['targetReturn', 'chosen', true], ['identity', 'chosen', false],
    ['constructVoid', 'chosen', false], ['constructAny', 'chosen', false],
    ['constructUnknown', 'chosen', true],
  ]],
  ['arity rejection preserves trailing reads, extra target reads and top-signature this reads', [
    ['arityEarly', 'same', false], ['arityTail', 'same', true],
    ['extraTarget', 'chosen', true], ['thisEarly', 'same', true],
    ['requiredVoid', 'chosen', false], ['requiredUndefined', 'same', false],
    ['optional', 'chosen', false],
  ]],
  ['structured comparison short circuits preserve inventory acquisition', [
    ['propertyFirst', 'same', false], ['callFirst', 'same', false],
    ['constructFirst', 'same', false], ['indexInventory', 'same', true],
  ]],
  ['signature matrices and explicit predicates retain ordered native decisions', [
    ['overloadSecond', 'chosen', false], ['overloadAll', 'chosen', false],
    ['overloadMismatch', 'same', false], ['predicateSame', 'chosen', false],
    ['predicateIndex', 'same', false], ['predicateBoolean', 'same', false],
    ['predicateMissing', 'same', true],
  ]],
  ['generic erasure, contextual instantiation and abstract constructors use actual factories', [
    ['genericContext', 'chosen', false], ['genericErase', 'chosen', false],
    ['abstractConcrete', 'same', false], ['concreteAbstract', 'chosen', false],
  ]],
];

for (const [description, cases] of groups) {
  test(description, async () => {
    for (const profile of ['tsconfig.json', 'loose.json']) {
      for (const [name, summary, missingVisited] of cases) {
        await compare(name, summary, missingVisited, profile);
      }
    }
  });
}

test('strict function variance exempts target methods and preserves callback covariance', async () => {
  for (const profile of ['tsconfig.json', 'loose.json', 'bivariant.json']) {
    await compare('functionVariance', profile === 'bivariant.json' ? 'chosen' : 'same', false, profile);
    await compare('methodVariance', 'chosen', false, profile);
    await compare('callbackVariance', 'same', false, profile);
  }
});

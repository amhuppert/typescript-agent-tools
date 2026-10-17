import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-heritage/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'cases.ts'), 'utf8');

async function compare(name, visited, profile) {
  const offset = source.indexOf(`${name}Result =`);
  assert.ok(offset >= 0, name);
  const at = {file: 'cases.ts', ...position(lineStarts(source), offset)};
  const config = path.join(directory, profile);
  const label = `${profile}/${name}`;
  const publicAnswer = await typeOracle(config, {at, properties: false});
  assert.equal(publicAnswer.summary, '"chosen"', label);
  assert.equal(publicAnswer.errorType, false, label);
  assert.deepEqual(publicAnswer.prelude, [], label);
  assert.deepEqual(publicAnswer.diagnostics, [], label);
  const answer = await compilerHookOracle(config, {operation: 'type', at});
  assert.equal(answer.summary, publicAnswer.summary, label);
  assert.equal(answer.errorType, false, label);
  assert.deepEqual(answer.beforeSummary.demandFailures, [], label);
  // Renamed unresolved names may select the native spelling-suggestion
  // diagnostic (2552), which does not change the consumed error type.
  assert.equal(answer.beforeSummary.diagnosticCodes.filter(code => code === 2304 || code === 2552).length,
    visited ? 1 : 0, `${label}: actual latent property demand`);
  const errorRead = answer.beforeSummary.observations.nonMissingSymbolErrorType;
  assert.equal(errorRead?.name, visited ? 'latent' : undefined,
    `${label}: native non-missing property type consumed errorType`);
  assert.ok(!answer.beforeSummary.diagnosticCodes.includes(2345), label);
}

test('inherited MapsThisOnly symbols preserve native thisless declaration distinctions', async () => {
  for (const profile of ['tsconfig.json', 'loose.json', 'exact.json']) {
    for (const [name, visited] of [
      ['shared', false], ['parenthesized', true], ['nested', true], ['union', true],
      ['methods', false], ['parameter', false], ['getter', false],
      ['methodParen', false], ['independent', true],
    ]) await compare(name, visited, profile);
  }
});

test('parenthesized type operations preserve child types and enclosing alias identity', async () => {
  for (const profile of ['tsconfig.json', 'loose.json', 'exact.json']) {
    for (const [name, summary] of [['wrappedRoot', 'number'], ['wrappedAliasRoot', 'WrappedAlias']]) {
      const offset = source.indexOf(`${name}:`);
      const at = {file: 'cases.ts', ...position(lineStarts(source), offset)};
      const config = path.join(directory, profile);
      const publicAnswer = await typeOracle(config, {at, properties: false});
      assert.equal(publicAnswer.summary, summary, `${profile}/${name}`);
      assert.equal(publicAnswer.errorType, false);
      const answer = await compilerHookOracle(config, {operation: 'type', at});
      assert.equal(answer.summary, summary, `${profile}/${name}`);
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.beforeSummary.demandFailures, []);
    }
  }
});

test('nonaugmenting bases bypass the property comparison that owns thisless shortcuts', async () => {
  for (const profile of ['tsconfig.json', 'loose.json', 'exact.json']) {
    await compare('emptyParents', false, profile);
  }
});

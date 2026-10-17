import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-structured-relations/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'sites.ts'), 'utf8');
const profiles = ['tsconfig.json', 'loose.json', 'exact.json'];
const cases = [
  ['unknownResult', '"chosen"', false],
  ['anyResult', '"chosen"', false],
  ['numberResult', '"chosen"', true],
  ['mismatchFirstResult', '"same"', false],
  ['mismatchLastResult', '"same"', true],
  ['identityResult', '"identity"', false],
  ['unmatchedResult', '"same"', false],
  ['optionalUnknownResult', '"chosen"', false],
  ['optionalRequiredResult', '"same"', false],
  ['sourceIndexResult', '"same"', true],
  ['targetIndexResult', '"same"', true],
  ['validIndexResult', '"same"', false],
  ['readonlyResult', '"chosen"', false],
];
const at = name => {
  const offset = source.indexOf(`${name} =`);
  assert.ok(offset >= 0, name);
  return { file: 'sites.ts', ...position(lineStarts(source), offset) };
};

test('fresh structured applicability preserves exact overload results across optional and null profiles', async () => {
  for (const profile of profiles) {
    for (const [name, summary] of cases) {
      const answer = await typeOracle(path.join(directory, profile), { at: at(name), properties: false });
      assert.equal(answer.kind, 'value', `${profile}:${name}`);
      assert.equal(answer.summary, summary, `${profile}:${name}`);
      assert.equal(answer.errorType, false, `${profile}:${name}`);
      assert.deepEqual(answer.prelude, []);
      assert.deepEqual(answer.diagnostics, []);
    }
  }
});

test('same-checker structured comparisons preserve target-first reads, false short circuits and inventory dependencies', async () => {
  for (const profile of profiles) {
    for (const [name, summary, missingVisited] of cases) {
      const answer = await compilerHookOracle(path.join(directory, profile), { operation: 'type', at: at(name) });
      assert.equal(answer.kind, 'value', `${profile}:${name}`);
      assert.equal(answer.summary, summary, `${profile}:${name}`);
      assert.equal(answer.errorType, false, `${profile}:${name}`);
      assert.deepEqual(answer.beforeSummary.demandFailures, []);
      // Each visited case consumes one missing property/index dependency in
      // the native schedule. Input checking does not pre-visit declarations.
      // Native noLib 2318s are
      // unrelated eager global diagnostics, not root-invalidity evidence.
      assert.equal(answer.beforeSummary.diagnosticCodes.filter(code => code === 2304).length,
        missingVisited ? 1 : 0, `${profile}:${name}: actual missing child demand`);
      assert.ok(!answer.beforeSummary.diagnosticCodes.includes(2345),
        `${profile}:${name}: a later applicable overload must avoid rejection recovery`);
    }
  }
});

test('actual call consumers distinguish depth overflow from admitted recursive Maybe and ordinary False', async () => {
  const limits = path.join(directory, 'limits');
  for (const name of ['depth99', 'depth100', 'depth101', 'activeMaybe', 'activeFalse']) {
    const input = fs.readFileSync(path.join(limits, `${name}.ts`), 'utf8');
    const at = { file: `${name}.ts`, ...position(lineStarts(input), input.indexOf('selected =')) };
    const overflow = name === 'depth100' || name === 'depth101';
    const summary = name === 'depth99' || name === 'activeMaybe' ? '"chosen"' : '"same"';
    for (const strict of [true, false]) {
      const config = path.join(limits, `${name}-${strict}.json`);
      const publicAnswer = await typeOracle(config, { at, properties: false });
      const answer = await compilerHookOracle(config, { operation: 'type', at });
      assert.equal(publicAnswer.summary, summary, `${name}:${strict}`);
      assert.equal(publicAnswer.errorType, false, `${name}:${strict}`);
      assert.deepEqual(publicAnswer.diagnostics, []);
      assert.equal(answer.summary, publicAnswer.summary, `${name}:${strict}`);
      assert.equal(answer.errorType, false, `${name}:${strict}`);
      assert.deepEqual(answer.beforeSummary.demandFailures, overflow ? ['relation_depth_limit'] : []);
      assert.equal(answer.beforeSummary.diagnosticCodes.includes(2321), overflow, `${name}:${strict}`);
      const first = answer.beforeSummary.observations.relationFirstFinish;
      assert.equal(first.overflow, overflow, `${name}:${strict}`);
      if (name.startsWith('depth')) {
        const maximum = name === 'depth99' ? 99 : 100;
        assert.equal(answer.beforeSummary.counters.relationSourceDepth, maximum, `${name}:${strict}`);
        assert.equal(answer.beforeSummary.counters.relationTargetDepth, maximum, `${name}:${strict}`);
        assert.equal(first.result, overflow ? 0 : -1, `${name}:${strict}`);
      } else {
        assert.deepEqual(answer.beforeSummary.observations.activeKeyMaybe,
          { allowance: 2000000, sourceDepth: 1, targetDepth: 1 }, `${name}:${strict}`);
        assert.equal(first.result, name === 'activeMaybe' ? 3 : 0, `${name}:${strict}`);
      }
    }
  }
});


test('unmatched properties use a valid actual global Object inventory across profiles', async () => {
  for (const profile of ['augmented.json', 'augmented-loose.json', 'augmented-exact.json']) {
    const request = { at: at('unmatchedResult'), properties: false };
    const publicAnswer = await typeOracle(path.join(directory, profile), request);
    const answer = await compilerHookOracle(path.join(directory, profile), {
      operation: 'type', at: request.at,
    });
    assert.equal(publicAnswer.summary, '"same"', profile);
    assert.equal(publicAnswer.errorType, false, profile);
    assert.deepEqual(publicAnswer.diagnostics, []);
    assert.equal(answer.summary, publicAnswer.summary, profile);
    assert.equal(answer.errorType, false, profile);
    assert.deepEqual(answer.beforeSummary.demandFailures, []);
    assert.equal(answer.beforeSummary.diagnosticCodes.filter(code => code === 2304).length, 0, profile);
    assert.equal(answer.beforeSummary.diagnosticCodes.filter(code => code === 2318).length, 9, profile);
    assert.equal(answer.beforeSummary.observations.relationFirstFinish.result, 0, profile);
    assert.equal(answer.beforeSummary.observations.relationFirstFinish.overflow, false, profile);
  }
});

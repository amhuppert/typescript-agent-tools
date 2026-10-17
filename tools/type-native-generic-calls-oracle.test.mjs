import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-generic-calls/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'sites.ts'), 'utf8');
const at = name => {
  const offset = source.indexOf(`${name} =`);
  assert.ok(offset >= 0, name);
  return { file: 'sites.ts', ...position(lineStarts(source), offset) };
};

test('fresh generic calls preserve explicit substitution, ordered defaults and native inference results', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [name, summary] of [
      ['stringResult', 'string'], ['literalResult', '"ready"'], ['numberResult', 'number'],
      ['unionResult', 'string | number'], ['booleanResult', 'boolean'], ['nullResult', 'null'],
      ['constrainedResult', 'string'], ['literalConstraintResult', '"ready"'],
      ['defaultedResult', 'number'], ['defaultOverrideResult', 'string'],
      ['dependentResult', 'string'], ['dependentOverrideResult', 'number'],
      ['relatedResult', '"ready"'], ['unionConstraintResult', 'number'],
      ['unknownConstraintResult', 'number'], ['unusedDefaultResult', 'number'],
      ['unusedDependentDefaultResult', 'string'], ['candidateResult', '"number"'],
      ['skippedConstraintResult', 'number'], ['forwardOverrideResult', 'string'],
      ['omittedBrokenConstraintResult', 'number'],
      ['inferredLiteralResult', '"ready"'], ['inferredConstrainedResult', '"ready"'],
      ['inferredPairResult', '"left" | "right"'], ['inferredDefaultResult', 'number'],
      ['inferredUnusedResult', 'unknown'],
    ]) {
      const answer = await typeOracle(path.join(directory, config), { at: at(name), properties: false });
      assert.equal(answer.kind, 'value');
      assert.equal(answer.summary, summary, `${config}:${name}`);
      assert.equal(answer.errorType, false, `${config}:${name}`);
      assert.deepEqual(answer.prelude, []);
    }
  }
});

test('same-checker generic calls distinguish rejected candidates, unvisited children and actual error defaults', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [name, summary, errorType, required, absent] of [
      ['unusedDefaultResult', 'number', false, [], [2304, 2344]],
      ['unusedDependentDefaultResult', 'string', false, [], [2304, 2344]],
      ['candidateResult', '"number"', false, [], [2304, 2344]],
      ['skippedConstraintResult', 'number', false, [], [2304, 2344]],
      ['forwardOverrideResult', 'string', false, [], [2304, 2344]],
      ['wrongConstraintResult', 'number', false, [2344], [2304]],
      ['wrongLiteralConstraintResult', '"other"', false, [2344], [2304]],
      ['wrongRelatedResult', 'number', false, [2344], [2304]],
      ['wrongArgumentResult', 'string', false, [2345], [2304, 2344]],
      ['rejectedConstraintBeforeMissingResult', 'number', false, [2344], [2304]],
      ['extraMissingArgumentResult', 'number', false, [2554], [2304]],
      ['missingTypeResult', 'MissingType', true, [2304], [2344]],
      ['missingArgumentResult', 'number', false, [2304], [2344]],
      ['missingConstraintResult', 'number', false, [2304], [2344]],
      ['missingDefaultResult', 'MissingDefault', true, [2304], [2344]],
      ['forwardDefaultResult', 'any', true, [], [2304, 2344]],
      ['omittedBrokenConstraintResult', 'number', false, [], [2304, 2344]],
      ['defaultBeforeConstraintResult', 'MissingDefault', true, [2304, 2344], []],
    ]) {
      const answer = await compilerHookOracle(path.join(directory, config), { operation: 'type', at: at(name) });
      assert.equal(answer.summary, summary, `${config}:${name}`);
      assert.equal(answer.errorType, errorType, `${config}:${name}`);
      assert.deepEqual(answer.beforeSummary.demandFailures, []);
      for (const code of required)
        assert.ok(answer.beforeSummary.diagnosticCodes.includes(code), `${config}:${name}: missing ${code}`);
      for (const code of absent)
        assert.ok(!answer.beforeSummary.diagnosticCodes.includes(code), `${config}:${name}: unexpected ${code}`);
      if (name === 'defaultBeforeConstraintResult')
        assert.ok(answer.beforeSummary.diagnosticCodes.indexOf(2304)
          < answer.beforeSummary.diagnosticCodes.indexOf(2344),
        `${config}:${name}: the default must be acquired before rejecting the supplied constraint`);
    }
  }
});

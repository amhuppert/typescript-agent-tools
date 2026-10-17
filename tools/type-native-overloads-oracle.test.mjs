import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-overloads/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'sites.ts'), 'utf8');
const at = name => {
  const offset = source.indexOf(`${name} =`);
  assert.ok(offset >= 0, name);
  return { file: 'sites.ts', ...position(lineStarts(source), offset) };
};

test('fresh native overload selection preserves specialization, subtype pass and declaration-parent order', async () => {
  for (const config of ['tsconfig.json', 'loose.json', 'reverse.json', 'reverse-loose.json']) {
    for (const [name, summary] of [
      ['exactResult', '"exact"'], ['escapedResult', '"exact"'],
      ['broadResult', '"broad"'], ['numberResult', '"number"'], ['trueResult', '"yes"'],
      ['anyResult', '"any"'], ['typedResult', '"number"'],
      ['oneResult', '"one"'], ['twoResult', '"two"'], ['omittedResult', '"void"'],
      ['thisResult', '"void"'], ['tieResult', '"first"'],
      ['callableResult', '"exact-call"'],
      ['groupedResult', config.startsWith('reverse') ? '"first-file"' : '"second-file"'],
    ]) {
      const answer = await typeOracle(path.join(root, config), { at: at(name), properties: false });
      assert.equal(answer.kind, 'value');
      assert.equal(answer.summary, summary, `${config}:${name}`);
      assert.equal(answer.errorType, false, `${config}:${name}`);
      assert.deepEqual(answer.prelude, []);
    }
  }
});

test('same-checker rejected overloads distinguish candidate failure from actually visited missing inputs', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [name, present, absent] of [
      ['failedResult', [2769], [2304]],
      ['wrongArityResult', [2554], [2304]],
      ['missingResult', [2304], []],
      ['extraMissingResult', [2554], [2304]],
      ['typeArgumentResult', [2558], [2304]],
      ['missingTypeArgumentResult', [2304, 2558], []],
    ]) {
      const answer = await compilerHookOracle(path.join(root, config), { operation: 'type', at: at(name) });
      assert.deepEqual(answer.beforeSummary.demandFailures, []);
      for (const code of present)
        assert.ok(answer.beforeSummary.diagnosticCodes.includes(code), `${config}:${name}: missing ${code}`);
      for (const code of absent)
        assert.ok(!answer.diagnosticCodes.includes(code), `${config}:${name}: unexpected ${code}`);
    }
  }
});

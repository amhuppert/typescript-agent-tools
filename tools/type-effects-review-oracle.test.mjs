// Independent native getTypeOfDottedName controls for the call-effect owner.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle, compilerHookOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';
import { scratchPath } from './harness-scratch.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-effects-review/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'effects.ts'), 'utf8');
function at(marker) {
  const comment = `/*${marker}*/ `;
  const offset = source.indexOf(comment);
  assert.notEqual(offset, -1);
  assert.equal(source.indexOf(comment, offset + 1), -1);
  return { file: 'effects.ts', ...position(lineStarts(source), offset + comment.length) };
}

test('dotted lookup unwraps parentheses and establishes absence for all other non-dotted syntax', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const marker of ['parenthesized', 'comma', 'nonnull', 'element', 'nested-call', 'arrow', 'conditional', 'number', 'string', 'object', 'pure-matching', 'pure-unknown', 'pure-nested', 'assigned-branches','dotted-method','dotted-property','dotted-missing','dotted-unannotated','dotted-parentheses','dotted-this','dotted-arrow-this','dotted-class-this','dotted-static-this']) {
      const answer = await typeOracle(path.join(directory, config), { at: at(marker), properties: false });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.history, 'baseline');
      assert.equal(answer.kind, 'expression');
      assert.equal(answer.summary, 'number', `${config}: ${marker}`);
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.prelude, []);
    }
  }
});

test('same-checker default-nil effect lookup consumes no callee type or call diagnostic', async () => {
  for (const marker of ['element', 'arrow', 'nested-call']) {
    const answer = await compilerHookOracle(path.join(directory, 'tsconfig.json'), { operation: 'type', at: at(marker) });
    assert.equal(answer.summary, 'number');
    assert.equal(answer.errorType, false);
    assert.equal(answer.beforeSummary.demandFailures.length, 0);
    assert(answer.beforeSummary.diagnosticCodes.every(code => code === 2318), marker);
  }
});

test('same-checker dotted effects retain consumed global dependencies', async () => {
  const observed = [];
  for (const marker of ['dotted-method', 'dotted-property', 'dotted-missing', 'dotted-unannotated', 'dotted-parentheses', 'dotted-this', 'dotted-arrow-this', 'dotted-class-this', 'dotted-static-this']) {
    const answer = await compilerHookOracle(path.join(directory, 'tsconfig.json'), { operation: 'type', at: at(marker) });
    assert.equal(answer.summary, 'number');
    assert.deepEqual(answer.beforeSummary.demandFailures, []);
    assert.deepEqual(answer.beforeSummary.diagnosticCodes, []);
    assert(!Object.values(answer.beforeSummary.observations).some(value => value.emptyRecovery === true));
    observed.push({ marker, summary: answer.summary, beforeSummary: answer.beforeSummary });
  }
  fs.writeFileSync(scratchPath('types-effects-dotted-consumption.json'), JSON.stringify(observed, null, 2));
});

// Missing noLib globals have a healthy-looking flow summary but property
// augmentation actually consumes the compiler's empty-object recovery sentinel.
test('missing Object is consumed only by the absent-property augmentation', async () => {
  for (const marker of ['dotted-method', 'dotted-missing']) {
    const answer = await compilerHookOracle(path.join(directory, 'missing.json'), { operation: 'type', at: at(marker) });
    assert.equal(answer.summary, 'number');
    assert.equal(answer.errorType, false);
    assert.deepEqual(answer.beforeSummary.observations['objectAugmentation:missing'], marker === 'dotted-missing' ? { emptyRecovery: true } : undefined);
  }
});

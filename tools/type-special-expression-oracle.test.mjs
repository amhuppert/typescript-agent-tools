import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle, compilerHookOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/special-expressions/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'cases.ts'), 'utf8');
const names = {
  constantNull: 'null', mutableNull: 'null', constantVoid: 'undefined', mutableVoid: 'undefined',
  deferredVoid: 'undefined', negatedNull: 'true', negatedVoid: 'true', positiveNull: 'number',
  negativeNull: 'number', invertedNull: 'number', positiveVoid: 'number', negativeVoid: 'number',
  invertedVoid: 'number', positiveBigint: 'number', localMutableNull: 'any', localMutableVoid: 'undefined',
  localConstantNull: 'null', localParenthesizedNull: 'any', localVarNull: 'any', exportedVarNull: 'null',
  localUninitialized: 'any', exportedUninitialized: 'any',
};

test('null and void variable types follow initializer widening and deferred operand checking', async () => {
  for (const config of ['tsconfig.json', 'loose.json', 'strict-no-implicit.json', 'loose-implicit.json']) {
    for (const [name, strictSummary] of Object.entries(names)) {
      const nullableLocal = ['localMutableNull', 'localParenthesizedNull', 'localVarNull'].includes(name);
      const original = config === 'strict-no-implicit.json' && nullableLocal ? 'null' : strictSummary;
      const summary = ['loose.json', 'loose-implicit.json'].includes(config) && ['null', 'undefined'].includes(original) ? 'any' : original;
      const answer = await typeOracle(path.join(directory, config), {
        at: { file: 'cases.ts', ...position(lineStarts(source), source.indexOf(name)) },
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.kind, 'value', name);
      assert.equal(answer.summary, summary, `${config}: ${name}`);
      assert.equal(answer.errorType, false, name);
      assert.deepEqual(answer.prelude, [], name);
    }
  }
});

test('the direct null token retains its expression type under both null profiles', async () => {
  const offset = source.indexOf('= null') + 2;
  for (const config of ['tsconfig.json', 'loose.json']) {
    const answer = await typeOracle(path.join(directory, config), {
      at: { file: 'cases.ts', ...position(lineStarts(source), offset) },
    });
    assert.equal(answer.kind, 'expression');
    assert.equal(answer.summary, 'null');
    assert.equal(answer.errorType, false);
  }
});

test('truthiness distinguishes complete anonymous shape from actual empty interface identity', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const name of ['negatedCallable', 'negatedConstructor', 'negatedIndexed', 'negatedAnonymous', 'negatedNominal']) {
      const answer = await typeOracle(path.join(directory, config), {
        at: { file: 'cases.ts', ...position(lineStarts(source), source.indexOf(name)) },
      });
      assert.equal(answer.summary, config === 'tsconfig.json' && name !== 'negatedAnonymous' ? 'false' : 'boolean', `${config}: ${name}`);
      assert.equal(answer.errorType, false);
      assert.equal(answer.kind, 'value');
    }
  }
});

test('nullable numeric unary roots retain consumed non-null recovery', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const name of ['positiveNull', 'negativeNull', 'invertedNull', 'positiveVoid', 'negativeVoid', 'invertedVoid']) {
      const answer = await compilerHookOracle(path.join(directory, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(name))},
        inspectNonNullReturns: true,
      });
      assert.equal(answer.summary, 'number');
      assert.equal(answer.errorType, false);
      const reads = Object.entries(answer.beforeSummary.observations).filter(([key]) => key.startsWith('nonNullReturn:'));
      assert.equal(reads.length, 1, name);
      assert.equal(reads[0][1].errorType, true, `${config}/${name}`);
    }
  }
});

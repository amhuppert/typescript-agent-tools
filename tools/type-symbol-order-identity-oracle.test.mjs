import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-symbol-order-identity/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const tag = '/*#value*/';
const at = { file: 'cases.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length) };

test('native actual prototype-symbol ties observe left first and reuse an earlier right observation', async () => {
  // This is an owner control. The unrelated scalar query checks that the
  // instrumentation has not manufactured a public type or changed its result.
  for (const symbolIDPrefix of [0, 8]) {
    for (const observeRightPrototypeFirst of [false, true]) {
      const answer = await compilerHookOracle(path.join(root, 'tsconfig.json'), {
        at, comparePrototypeSymbols: true, observeRightPrototypeFirst,
        symbolIDPrefix, singleThreadBinding: true,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.summary, '0');
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.beforeSummary.demandFailures, []);
      assert.deepEqual(answer.prototypeComparison, {
        forward: observeRightPrototypeFirst ? 1 : -1,
        reverse: observeRightPrototypeFirst ? -1 : 1,
        same: 0, name: 'prototype',
      });
    }
  }
});

import assert from 'node:assert/strict';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-nullable/', import.meta.url));
for (const strict of [true, false]) {
  test(`native optional receivers consume the global NonNullable alias (${strict})`, async () => {
    for (const variant of ['missing', 'kind', 'arity', 'zero', 'custom', 'identity', 'intersection', 'broken']) {
      const answer = await typeOracle(path.join(root, `${variant}${strict ? '' : '-loose'}.json`), {
        at: {file: 'cases.ts', line: 2, column: 14}, properties: false,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.summary, strict && variant === 'custom' ? '"custom" | undefined' : 'any', variant);
      assert.equal(answer.errorType, strict && variant === 'broken', variant);
      assert.deepEqual(answer.prelude, [], variant);
    }
  });
}

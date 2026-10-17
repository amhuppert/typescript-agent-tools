import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/symbol-written-names/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'input.ts'), 'utf8');
const cases = [
  ['bare', 'T'], ['quoted', '"T"'], ['quotedEscape', '"\\u0054"'], ['identifierEscape', '\\u0054'], ['computed', '["T"]'],
];

test('native symbol-written name uses the first own named association across merged roles', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [markerName, expected] of cases) {
      const marker = `/*#${markerName}*/`;
      const answer = await typeOracle(path.join(directory, config), {
        at: { file: 'input.ts', ...position(lineStarts(source), source.indexOf(marker) + marker.length) },
        properties: false,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.history, 'baseline');
      assert.equal(answer.kind, 'declared');
      assert.equal(answer.summary, expected, `${config} ${markerName}`);
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.prelude, []);
    }
  }
});

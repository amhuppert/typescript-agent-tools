import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-optional-flow/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'optional.ts'), 'utf8');
const markers = [...source.matchAll(/\/\*([^*]+)\*\/ /g)].map(match => ({
  marker: match[1], offset: match.index + match[0].length,
}));

test('native optional-chain forms preserve unrelated identifier identity', async () => {
  assert.equal(markers.length, 13);
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const { marker, offset } of markers) {
      const answer = await typeOracle(path.join(directory, config), {
        at: { file: 'optional.ts', ...position(lineStarts(source), offset) }, properties: false,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.history, 'baseline');
      assert.equal(answer.kind, 'expression', `${config}: ${marker}`);
      assert.equal(answer.summary, 'number', `${config}: ${marker}`);
      assert.equal(answer.errorType, false, `${config}: ${marker}`);
      assert.deepEqual(answer.prelude, []);
    }
  }
});

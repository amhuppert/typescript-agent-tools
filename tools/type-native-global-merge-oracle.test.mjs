import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

// Global interfaces declared in two script files merge, including their type
// parameters: `Array<T>` and `Box<T>` keep one parameter each, so anonymous
// member types and `this` in either declaration instantiate with it. Overloads
// of one global function declared in two files share their merged symbol when
// candidates are reordered. A computed getter's contextual return type comes
// from the setter of its late-bound symbol.
const root = fileURLToPath(new URL('../fixtures/types/native-global-merge/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'main.ts'), 'utf8');
const expected = {
  array: 'number[]',
  at: 'number | undefined',
  merged: 'string',
  own: 'string',
  firstNested: 'string',
  secondNested: 'string',
  callback: '(value: string) => void',
  self: 'Box<string>',
  element: 'number',
  picked: '"second-number"',
  green: 'Color.Green',
  literalGetter: 'number',
  symbolGetter: 'string',
  plainGetter: 'boolean',
};

test('cross-file global interface merges keep one type-parameter list', async () => {
  const seen = [];
  for (const config of ['tsconfig.json', 'nocheck.json']) {
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file: 'main.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const answer = await typeOracle(path.join(root, config), {at, properties: false});
      assert.equal(answer.summary, expected[marker[1]], `${config}: ${marker[1]}`);
      assert.equal(answer.errorType, false, `${config}: ${marker[1]}`);
      assert.equal(answer.compiler, '7.0.2');
      seen.push(marker[1]);
    }
  }
  assert.deepEqual(seen, [...Object.keys(expected), ...Object.keys(expected)]);
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-capture-flow/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'flow.ts'), 'utf8');
const union = new Set(['before-last', 'declaration-after-last', 'constant-refined-declaration',
  'compound-inside', 'compound-after', 'nested-assignment', 'class-declaration-method',
  'class-declaration-getter', 'class-declaration-setter', 'exported-let', 'module-local-let',
  'parameter-before-last']);
const string = new Set(['after-last', 'function-after-last', 'parameter-after-last']);
const controls = [...source.matchAll(/\/\*([^*]+)\*\/ /g)].map(match => ({
  marker: match[1], offset: match.index + match[0].length,
}));

test('native constant, parameter and last-assignment capture identities', async () => {
  assert.equal(controls.length, 33);
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const { marker, offset } of controls) {
      const answer = await typeOracle(path.join(directory, config), {
        at: { file: 'flow.ts', ...position(lineStarts(source), offset) }, properties: false,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.history, 'baseline');
      assert.equal(answer.kind, 'expression', `${config}: ${marker}`);
      assert.equal(answer.summary, union.has(marker) ? 'string | number' : string.has(marker) ? 'string' : 'number',
        `${config}: ${marker}`);
      assert.equal(answer.errorType, false, `${config}: ${marker}`);
      assert.deepEqual(answer.prelude, []);
    }
  }
});

test('native definite assignments differ from compound assignments and unrelated diagnostics', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const marker of ['never-initialized', 'compound-not-definite', 'definitely-assigned-before', 'definitely-assigned-after']) {
      const { offset } = controls.find(control => control.marker === marker);
      const at = position(lineStarts(source), offset);
      const answer = await typeOracle(path.join(directory, config), {
        at: { file: 'flow.ts', ...at }, properties: false, diagnostics: true,
      });
      const usedBefore = answer.diagnostics.filter(diagnostic => diagnostic.code === 2454
        && diagnostic.location?.file === 'flow.ts'
        && diagnostic.location.start.line === at.line
        && diagnostic.location.start.column === at.column);
      assert.equal(usedBefore.length, config === 'tsconfig.json'
        && ['never-initialized', 'compound-not-definite'].includes(marker) ? 1 : 0,
      `${config}: ${marker}`);
      assert.equal(answer.summary, 'number');
      assert.equal(answer.errorType, false);
    }
  }
});

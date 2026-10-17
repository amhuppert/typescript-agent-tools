import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-basic-flow/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'flow.ts'), 'utf8');
const controls = [...source.matchAll(/\/\*([^*]+)\*\/ /g)].map(match => ({
  marker: match[1], offset: match.index + match[0].length,
}));

test('native default/delete/destructuring/call-mutation graph controls', async () => {
  assert.equal(controls.length, 28);
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const { marker, offset } of controls) {
      const answer = await typeOracle(path.join(directory, config), {
        at: { file: 'flow.ts', ...position(lineStarts(source), offset) }, properties: false,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.history, 'baseline');
      assert.equal(answer.kind, 'expression', `${config}: ${marker}`);
      const summary = /effect|first-declared|ordinary-pattern-key/.test(marker) ? 'number | "ready"'
        : /matching|pure/.test(marker) ? 'string | number' : 'number';
      assert.equal(answer.summary, summary, `${config}: ${marker}`);
      assert.equal(answer.errorType, false, `${config}: ${marker}`);
      assert.deepEqual(answer.prelude, []);
    }
  }
});

test('native mutation joins restore actual declared reference identity', async () => {
  const observations = [];
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const marker of ['push-effect', 'unshift-effect', 'escaped-push-effect', 'element-effect', 'first-declared-branch']) {
      const { offset } = controls.find(control => control.marker === marker);
      const end = offset + 'guarded'.length;
      const answer = await compilerHookOracle(path.join(directory, config), {
        at: { file: 'flow.ts', ...position(lineStarts(source), offset) }, properties: false,
      });
      const witness = Object.entries(answer.beforeSummary.observations).find(([key, value]) =>
        key.startsWith('referenceFlow:') && value.referenceEnd === end)?.[1];
      observations.push({ config, marker, witness });
      fs.writeFileSync('.cc/temp/native-basic-flow-identity.json', JSON.stringify(observations, null, 2));
      assert.equal(answer.summary, 'number | "ready"', `${config}: ${marker}`);
      assert.equal(witness?.sameDeclared, true, `${config}: ${marker}`);
      assert.equal(witness?.errorType, false, `${config}: ${marker}`);
    }
  }
});

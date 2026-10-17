import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-new-type-flags/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');

test('native fresh generic objects retain substituted property and method results', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const marker of ['nested', 'direct', 'constrained', 'method', 'noInferKeys', 'emptyKeys', 'runnerKeys', 'mappedKeys']) {
      const tag = `/*#${marker}*/`;
      const offset = source.indexOf(tag) + tag.length;
      const answer = await typeOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), offset)},
      });
      assert.equal(answer.compiler, '7.0.2');
      const expected = {noInferKeys: '"value"', mappedKeys: '"value"', emptyKeys: 'never', runnerKeys: '"run"'}[marker] ?? 'string';
      assert.equal(answer.summary, expected, `${config}/${marker}`);
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.diagnostics, []);
    }
  }
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/circular-aliases/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'types.ts'), 'utf8');

test('self, mutual and consumed outer alias cycles return compiler error types in both null profiles', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const name of ['Loop', 'First', 'Second', 'Outer', 'broken']) {
      const marker = name === 'broken' ? `let ${name}` : `type ${name}`;
      const answer = await typeOracle(path.join(directory, config), {
        at: { file: 'types.ts', ...position(lineStarts(source), source.indexOf(marker) + marker.indexOf(name)) },
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.kind, name === 'broken' ? 'value' : 'declared');
      assert.equal(answer.summary, 'any', `${config}: ${name}`);
      assert.equal(answer.errorType, true, `${config}: ${name}`);
    }
  }
});

test('a deferred recursive object remains healthy rather than circular alias recovery', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const name of ['Healthy', 'healthy']) {
      const answer = await typeOracle(path.join(directory, config), {
        at: { file: 'types.ts', ...position(lineStarts(source), source.indexOf(name)) },
      });
      assert.equal(answer.kind, name === 'healthy' ? 'value' : 'declared');
      assert.equal(answer.summary, 'Healthy');
      assert.equal(answer.errorType, false);
    }
  }
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-applicable-index-symbol/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
test('native applicable index candidates remain unrelated to a healthy member seed', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const marker of ['seed', 'healthy']) {
      const tag = `/*#${marker}*/`;
      const answer = await compilerHookOracle(path.join(root, config), {operation: 'member', at: {file:'cases.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length)}});
      fs.writeFileSync(`.cc/temp/applicable-index-symbol-${config}-${marker}.json`, JSON.stringify(answer, null, 2));
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.checkerCount, 1);
      assert.equal(answer.history, 'baseline');
      assert.deepEqual(answer.demandFailures, []);
      assert.equal(answer.groups.length, 1);
      assert.equal(answer.groups[0].occurrences.length, 2);
      const candidates = answer.candidateObservations.filter(row => row.location.start.line >= 7);
      assert.equal(candidates.length, 4);
      assert.ok(candidates.every(row => row.relatedOrigins.length === 0));
    }
  }
});

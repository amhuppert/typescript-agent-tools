import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = path.resolve('fixtures/types/native-relation-key-identity');
test('warm generic comparisons preserve overload rejection for distinct keys', async () => {
  const rows = [];
  const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
  fs.mkdirSync('.cc/temp', {recursive: true});
  for (const config of ['strict.json', 'nocheck.json']) {
    for (const kind of ['parameter', 'target', 'nested', 'constrained']) {
      const marker = kind + 'Chosen';
      const answer = await compilerHookOracle(path.join(root, config), {
        history: 'cold', inspectRelationKeys: true,
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(marker + ' ='))},
      });
      fs.writeFileSync(`.cc/temp/relation-key-identity-${config}-${marker}.json`, JSON.stringify(answer, null, 2)+'\n');
      assert.equal(answer.checkerCount, 1);
      assert.equal(answer.summary, 'number', `${config}/${marker}`);
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.demandFailures, []);
      assert.ok(answer.beforeSummary.counters.genericKeyBuilds >= 2, marker);
      rows.push({config, marker, counters: answer.beforeSummary.counters,
        compiler: answer.compiler, sourceCommit: answer.sourceCommit, inputHashes: answer.inputHashes});
      fs.writeFileSync('.cc/temp/native-relation-key-identity.json', JSON.stringify(rows, null, 2)+'\n');
    }
  }
});

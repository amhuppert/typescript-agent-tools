import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = path.resolve('fixtures/types/native-promise-cache-resources');
test('native promised and awaited caches retain distinct fixed type identities', async () => {
  const rows = [];
  for (const config of ['strict', 'nocheck', 'loose', 'loose-nocheck']) {
    for (const [file, width] of [['small.ts', 16], ['wide.ts', 64]]) {
      const source = fs.readFileSync(path.join(root, file), 'utf8');
      const answer = await compilerHookOracle(path.join(root, `${config}.json`), {
        history: 'cold', inspectPromiseCacheSlots: true,
        at: {file, ...position(lineStarts(source), source.indexOf('const chosen') + 6)},
      });
      const slots = Object.fromEntries(Object.entries(answer.beforeSummary.observations)
        .filter(([key]) => key.startsWith('promiseCache:')));
      rows.push({config, file, width, summary: answer.summary, slots,
        diagnosticCodes: answer.beforeSummary.diagnosticCodes, compiler: answer.compiler,
        sourceCommit: answer.sourceCommit, inputHashes: answer.inputHashes});
      fs.writeFileSync('.cc/temp/native-promise-cache-resources.json', JSON.stringify(rows, null, 2)+'\n');
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.checkerCount, 1);
      assert.equal(answer.summary, 'number');
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.demandFailures, []);
      assert.deepEqual(answer.beforeSummary.diagnosticCodes, [2695]);
      for (const kind of ['promised', 'awaited']) {
        const selected = Object.entries(slots).filter(([key]) => key.startsWith(`promiseCache:${kind}:`));
        assert.equal(selected.length, kind === 'awaited' ? 2 * width : width);
        assert.ok(selected.every(([, slot]) => Number.isInteger(slot.typeId) && Number.isInteger(slot.resultId)));
      }
    }
  }
});

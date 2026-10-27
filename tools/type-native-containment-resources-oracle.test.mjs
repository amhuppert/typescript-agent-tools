import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';
const root = path.resolve('fixtures/types/native-containment-resources');
test('containment traverses native reference arguments and flat alternatives', async () => {
  const rows = [];
  for (const config of ['strict', 'nocheck', 'loose', 'loose-nocheck']) {
    for (const file of ['deep-small.ts', 'deep-wide.ts', 'flat-small.ts', 'flat-wide.ts']) {
      const source = fs.readFileSync(path.join(root, file), 'utf8');
      for (const marker of [file.startsWith('deep') ? 'deepChosen' : 'flatChosen']) {
        const answer = await compilerHookOracle(path.join(root, `${config}.json`), {
          history: 'cold', inspectContainment: true,
          at: {file, ...position(lineStarts(source), source.indexOf(`const ${marker}`) + 6)},
        });
        rows.push({config, file, marker, summary: answer.summary, counters: answer.beforeSummary.counters,
          diagnosticCodes: answer.beforeSummary.diagnosticCodes, compiler: answer.compiler,
          sourceCommit: answer.sourceCommit, inputHashes: answer.inputHashes});
        fs.writeFileSync(scratchPath('native-containment-resources.json'), JSON.stringify(rows, null, 2)+'\n');
        assert.equal(answer.compiler, '7.0.2');
        assert.equal(answer.checkerCount, 1);
        assert.equal(answer.summary, 'number');
        assert.equal(answer.errorType, false);
        assert.deepEqual(answer.demandFailures, []);
        assert.deepEqual(answer.beforeSummary.diagnosticCodes, []);
        const c = answer.beforeSummary.counters;
        const count = marker === 'deepChosen' ? (file.endsWith('small.ts') ? 8 : 24) : (file.endsWith('small.ts') ? 16 : 64);
        assert.equal(c.containmentDepth, 0);
        assert.equal(c.containmentPeakDepth, marker === 'deepChosen' ? count + 1 : 2);
        assert.equal(c.containmentCalls, marker === 'deepChosen' ? 2 * count + 1 : 2 * count + 5);
      }
    }
  }
});

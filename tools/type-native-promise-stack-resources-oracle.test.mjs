import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

test('native awaited stack grows with nesting and stays shallow for flat width', async () => {
  const rows = [];
  for (const config of ['strict', 'nocheck', 'loose', 'loose-nocheck']) {
    for (const [family, file, size] of [
      ['stack', 'small.ts', 8], ['stack', 'deep.ts', 32],
      ['stack', 'union-small.ts', 4], ['stack', 'union-deep.ts', 16],
      ['cache', 'small.ts', 16], ['cache', 'wide.ts', 64],
    ]) {
      const root = path.resolve(`fixtures/types/native-promise-${family}-resources`);
      const source = fs.readFileSync(path.join(root, file), 'utf8');
      const answer = await compilerHookOracle(path.join(root, `${config}.json`), {
        history: 'cold', inspectPromiseTraversal: true,
        at: {file, ...position(lineStarts(source), source.indexOf('const chosen') + 6)},
      });
      const counters = Object.fromEntries(Object.entries(answer.beforeSummary.counters)
        .filter(([key]) => key.startsWith('promiseTraversal:')));
      rows.push({config, family, file, size, summary: answer.summary, counters,
        diagnosticCodes: answer.beforeSummary.diagnosticCodes, compiler: answer.compiler,
        sourceCommit: answer.sourceCommit, inputHashes: answer.inputHashes});
      fs.writeFileSync('.cc/temp/native-promise-stack-resources.json', JSON.stringify(rows, null, 2)+'\n');
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.checkerCount, 1);
      assert.equal(answer.summary, 'number');
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.demandFailures, []);
      assert.deepEqual(answer.beforeSummary.diagnosticCodes, [2695]);
      const union = file.startsWith('union-');
      assert.equal(counters['promiseTraversal:awaitedPeakDepth'], family === 'stack' ? (union ? 2 * size + 1 : size + 1) : 2);
      for (const kind of ['promised', 'awaited']) {
        const calls = family === 'cache' ? 2 * size : union ? (kind === 'awaited' ? 3 * size + 1 : 2 * size + 1) : size + 1;
        assert.equal(counters[`promiseTraversal:${kind}Calls`], calls);
        assert.equal(counters[`promiseTraversal:${kind}Depth`], 0);
      }
      const comparisons = family === 'cache' ? 0 : union ? 2 * size * size - size : size * (size - 1) / 2;
      assert.equal(counters['promiseTraversal:stackComparisons'] ?? 0, comparisons);
    }
  }
});

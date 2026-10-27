import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-await-context/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
test('native await operand contexts preserve absent, annotated, return and nil states', async () => {
  for (const config of ['tsconfig.json', 'loose.json', 'missing.json']) {
    for (const marker of ['absent', 'annotation', 'return', 'nil']) {
      const tag = `/*#${marker}*/`;
      const answer = await typeOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(tag)+tag.length)},
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.summary, 'number', `${config}/${marker}`);
      assert.equal(answer.errorType, false, `${config}/${marker}`);
      const native = await compilerHookOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(tag)+tag.length)},
        inspectAwaitContexts: true,
      });
      assert.deepEqual([native.summary, native.errorType], [answer.summary, answer.errorType]);
      assert.equal(native.checkerCount, 1);
      const observations = native.beforeSummary.observations;
      assert.equal(observations.awaitContext.nil, marker === 'absent', `${config}/${marker}`);
      if (marker === 'absent') assert.equal(observations.awaitContextResult, undefined);
      else assert.equal(observations.awaitContextResult.nil, marker === 'nil', `${config}/${marker}`);
      if (marker === 'absent' || marker === 'nil') assert.equal(observations.promiseLikeFactory, undefined);
      else assert.equal(observations.promiseLikeFactory.emptyRecovery, config === 'missing.json');
      const recovery = config === 'missing.json' && ['annotation', 'return'].includes(marker);
      assert.deepEqual(native.beforeSummary.diagnosticCodes, recovery ? [2318] : []);
      fs.writeFileSync(scratchPath(`await-context-${config}-${marker}.json`), JSON.stringify(native, null, 2)+'\n');
    }
  }
});

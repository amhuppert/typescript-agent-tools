import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-recovery-cause-limit/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
test('native comma result stays healthy after 16, 17 and 18 consumed numeric error types', async () => {
  for (const match of source.matchAll(/\/\*#causes(\d+)\*\//g)) {
    const at = {file: 'cases.ts', ...position(lineStarts(source), match.index + match[0].length)};
    const native = await compilerHookOracle(path.join(root, 'tsconfig.json'), {at});
    const api = await typeOracle(path.join(root, 'tsconfig.json'), {at, properties: false});
    assert.equal(native.compiler, '7.0.2');
    assert.deepEqual([native.summary, native.errorType], ['1', false]);
    assert.deepEqual([api.summary, api.errorType], ['1', false]);
    assert.deepEqual(native.beforeSummary.diagnosticCodes,
      [2365, ...Array(Number(match[1]) + 1).fill(2695), ...Array(Number(match[1])).fill(2365)]);
  }
});

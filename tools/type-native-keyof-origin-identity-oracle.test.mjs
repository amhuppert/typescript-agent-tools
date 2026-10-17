import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = path.resolve('fixtures/types/native-keyof-origin-identity');
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const expected = JSON.parse(fs.readFileSync(path.join(root, 'expected.json'), 'utf8'));
test('keyof origins remain distinct from equal key sets and plain literal unions', async () => {
  const rows = [];
  fs.mkdirSync('.cc/temp', {recursive: true});
  for (const profile of ['strict', 'loose', 'nocheck']) for (const [marker, summary] of Object.entries(expected)) {
    const answer = await compilerHookOracle(path.join(root, profile + '.json'), {
      history: 'cold',
      at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(`const ${marker}Chosen`) + 6)},
    });
    rows.push({profile, marker, summary: answer.summary, errorType: answer.errorType,
      diagnosticCodes: answer.beforeSummary.diagnosticCodes,
      compiler: answer.compiler, sourceCommit: answer.sourceCommit, inputHashes: answer.inputHashes});
    fs.writeFileSync('.cc/temp/native-keyof-origin-identity.json', JSON.stringify(rows, null, 2) + '\n');
    assert.equal(answer.compiler, '7.0.2');
    assert.equal(answer.checkerCount, 1);
    assert.equal(answer.summary, summary);
    assert.equal(answer.errorType, false);
    assert.deepEqual(answer.demandFailures, []);
    assert.deepEqual(answer.beforeSummary.diagnosticCodes, [2695]);
  }
});

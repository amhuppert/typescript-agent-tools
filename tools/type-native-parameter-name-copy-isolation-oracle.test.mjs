import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = path.resolve('fixtures/types/native-parameter-name-copy-isolation');
const families = JSON.parse(fs.readFileSync(path.join(root, 'cases.json'), 'utf8'));
test('normalized parameter names stay separate from equal-growth written names', async () => {
  const rows = [];
  fs.mkdirSync('.cc/temp', {recursive: true});
  for (const config of ['strict.json', 'nocheck.json']) for (const file of ['small.ts', 'wide.ts']) {
    const source = fs.readFileSync(path.join(root, file), 'utf8');
    for (const family of families) for (const suffix of ['Cheap', 'Chosen']) {
      const marker = family + suffix;
      const answer = await compilerHookOracle(path.join(root, config), {
        history: 'cold',
        at: {file, ...position(lineStarts(source), source.indexOf(`const ${marker}`) + 6)},
      });
      rows.push({config, file, marker, summary: answer.summary,
        counters: answer.beforeSummary.counters, diagnosticCodes: answer.beforeSummary.diagnosticCodes,
        compiler: answer.compiler, sourceCommit: answer.sourceCommit, inputHashes: answer.inputHashes});
      fs.writeFileSync('.cc/temp/native-parameter-name-copy-isolation.json', JSON.stringify(rows, null, 2)+'\n');
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.checkerCount, 1);
      assert.equal(answer.summary, 'number');
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.demandFailures, []);
      assert.deepEqual(answer.beforeSummary.diagnosticCodes, []);
    }
  }
});


test('native parameter displays retain the first computed member spelling', async () => {
  const file = 'small.ts';
  const source = fs.readFileSync(path.join(root, file), 'utf8');
  const characters = {ascii: 'A', unicode: 'Σ', supplementary: '𝒜', escaped: 'A', escapedSupplementary: '𝒜', underscores: '_'};
  for (const config of ['strict.json', 'nocheck.json']) {
    for (const family of families) for (const suffix of ['Cheap', 'Chosen']) {
      const prefix = `interface Copy${suffix}${family}16<`;
      const offset = source.indexOf(prefix) + prefix.length;
      assert.ok(offset >= prefix.length);
      const c = characters[family];
      const expected = suffix === 'Cheap' ? `["${c}"/*${c.repeat(16)}*/]` : `["${c.repeat(16)}"]`;
      const answer = await compilerHookOracle(path.join(root, config), {
        history: 'cold', at: {file, ...position(lineStarts(source), offset)},
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.summary, expected, `${config}/${family}/${suffix}`);
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.demandFailures, []);
    }
  }
});

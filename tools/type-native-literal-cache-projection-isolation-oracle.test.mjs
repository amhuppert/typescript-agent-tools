import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = path.resolve('fixtures/types/native-literal-cache-projection-isolation');
test('equal literal projection payloads isolate warmed native cache work', async () => {
  const rows = [];
  fs.mkdirSync('.cc/temp', {recursive: true});
  for (const config of ['strict.json', 'nocheck.json']) for (const file of ['small.ts', 'wide.ts']) {
    const source = fs.readFileSync(path.join(root, file), 'utf8');
    for (const family of ['ascii', 'unicode', 'surrogate', 'template']) for (const suffix of ['Cheap', 'Chosen']) {
      const marker = family + suffix;
      const answer = await compilerHookOracle(path.join(root, config), {
        history: 'cold', inspectLiteralCaches: true, inspectConstantTemplates: true,
        at: {file, ...position(lineStarts(source), source.indexOf(`const ${marker}`) + 6)},
      });
      rows.push({config, file, marker, summary: answer.summary,
        counters: answer.beforeSummary.counters, diagnosticCodes: answer.beforeSummary.diagnosticCodes,
        compiler: answer.compiler, sourceCommit: answer.sourceCommit, inputHashes: answer.inputHashes});
      fs.writeFileSync('.cc/temp/native-literal-cache-projection-isolation.json', JSON.stringify(rows, null, 2)+'\n');
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.checkerCount, 1);
      assert.equal(answer.summary, 'number', `${config}/${file}/${marker}`);
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.demandFailures, []);
      assert.deepEqual(answer.beforeSummary.diagnosticCodes.toSorted((a,b) => a-b),
        suffix === 'Cheap' ? [2474, 2695, 2695] : Array(10).fill(2695));
      assert.equal(answer.beforeSummary.counters.constantTemplateSpans ?? 0, suffix === 'Cheap' ? 1 : 0);
      assert.equal(answer.beforeSummary.counters.checkedTemplateExpressions ?? 0, 0);
    }
  }
  for (const config of ['strict.json', 'nocheck.json']) for (const file of ['small.ts', 'wide.ts']) {
    for (const family of ['ascii', 'unicode', 'surrogate', 'template']) {
      const get = suffix => rows.find(row => row.config === config && row.file === file && row.marker === family + suffix).counters;
      const cheap = get('Cheap'), chosen = get('Chosen');
      assert.equal(chosen.stringLiteralLookups - cheap.stringLiteralLookups, 8);
      assert.equal(chosen.stringLiteralMisses, cheap.stringLiteralMisses);
      assert.equal(chosen.stringLiteralLookups - chosen.stringLiteralMisses, 8);
      const width = file === 'small.ts' ? 16 : 256;
      const bytes = family === 'unicode' ? 2 : family === 'surrogate' ? 3 : 1;
      assert.equal(chosen.stringLiteralKeyBytes - cheap.stringLiteralKeyBytes,
        8 * width * bytes, `${config}/${file}/${family}`);
    }
  }
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-generated-literal-context/', import.meta.url));
for (const group of ['context', 'cold', 'warm']) {
  const file = group === 'context' ? 'cases.ts' : `${group}.ts`;
  const source = fs.readFileSync(path.join(root, file), 'utf8');
  for (const mode of ['strict', 'loose']) {
    const profile = `${group}-${mode}`;
    test(`native generated literal context (${profile})`, async () => {
      const config = path.join(root, `${profile}.json`);
      const rows = [];
      for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
        const at = {file, ...position(lineStarts(source), marker.index + marker[0].length)};
        const native = await compilerHookOracle(config, {at, inspectDiagnosticCache: true});
        const api = await typeOracle(config, {at, properties: false});
        assert.equal(native.compiler, '7.0.2');
        assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
        rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
          codes: native.beforeSummary.diagnosticCodes,
          demandFailures: native.beforeSummary.demandFailures,
          acquisitions: native.beforeSummary.displayAcquisitions ?? []});
      }
      assert.equal(rows.length, group === 'context' ? 12 : 3);
      fs.mkdirSync('.cc/temp', {recursive: true});
      fs.writeFileSync(`.cc/temp/native-generated-literal-context-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
      assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
    });
  }
}

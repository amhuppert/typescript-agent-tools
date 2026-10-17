import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-late-symbol-tables/', import.meta.url));
for (const profile of ['tsconfig', 'loose']) {
  test(`native resolved member and export tables (${profile})`, async () => {
    const rows = [];
    for (const file of ['names.ts', 'merges.ts', 'indexes.ts', 'recovery.ts']) {
      const source = fs.readFileSync(path.join(root, file), 'utf8');
      for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
        const at = {file, ...position(lineStarts(source), marker.index + marker[0].length)};
        const config = path.join(root, `${profile}.json`);
        const native = await compilerHookOracle(config, {at, inspectDiagnosticCache: true});
        const api = await typeOracle(config, {at, properties: false});
        assert.equal(native.compiler, '7.0.2');
        assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
        rows.push({file, name: marker[1], summary: native.summary, errorType: native.errorType,
          codes: native.beforeSummary.diagnosticCodes,
          demandFailures: native.beforeSummary.demandFailures,
          acquisitions: native.beforeSummary.displayAcquisitions ?? []});
      }
    }
    assert.equal(rows.length, 42);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-late-symbol-tables-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    const expected = path.join(root, `expected-${profile}.json`);
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(expected, 'utf8')));
  });
}

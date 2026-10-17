import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-module-scope/', import.meta.url));
for (const profile of ['strict', 'loose']) {
  test(`native module-scope lookup (${profile})`, async () => {
    const rows = [];
    for (const file of ['named.ts', 'default-interface.ts', 'default-class.ts', 'default-function.ts', 'reexports.ts', 'ambient.ts']) {
      const source = fs.readFileSync(path.join(root, file), 'utf8');
      for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
        const at = {file, ...position(lineStarts(source), marker.index + marker[0].length)};
        const config = path.join(root, `${profile}.json`);
        const native = await compilerHookOracle(config, {at, inspectDiagnosticCache: true});
        const api = await typeOracle(config, {at, properties: false});
        assert.equal(native.compiler, '7.0.2');
        assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
        assert.equal(native.errorType, false, marker[1]);
        assert.deepEqual(native.beforeSummary.diagnosticCodes, [], marker[1]);
        assert.deepEqual(native.beforeSummary.demandFailures, [], marker[1]);
        rows.push({file, name: marker[1], summary: native.summary, errorType: native.errorType,
          codes: native.beforeSummary.diagnosticCodes,
          demandFailures: native.beforeSummary.demandFailures});
      }
    }
    assert.equal(rows.length, 23);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-module-scope-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

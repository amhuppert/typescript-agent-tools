import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-string-names/', import.meta.url));
const cases = JSON.parse(fs.readFileSync(path.join(root, 'cases.json'), 'utf8'));
for (const profile of ['strict', 'loose']) {
  test(`native exact cooked identity names (${profile})`, async () => {
    const config = path.join(root, `${profile}.json`);
    const rows = [];
    for (const entry of cases) {
      const source = fs.readFileSync(path.join(root, entry.file), 'utf8');
      const marker = '/*#result*/';
      const at = {file:entry.file, ...position(lineStarts(source), source.indexOf(marker) + marker.length)};
      const native = await compilerHookOracle(config, {at, inspectDiagnosticCache:true});
      const api = await typeOracle(config, {at, properties:false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary,native.errorType], [api.summary,api.errorType], entry.name);
      assert.equal(native.summary, entry.summary, entry.name);
      assert.equal(native.errorType, false, entry.name);
      assert.deepEqual(native.beforeSummary.diagnosticCodes, [], entry.name);
      assert.deepEqual(native.beforeSummary.demandFailures, [], entry.name);
      rows.push({...entry, location:api.location, origins:api.origins,
        codes:native.beforeSummary.diagnosticCodes, demandFailures:native.beforeSummary.demandFailures});
    }
    assert.equal(rows.length, 27);
    fs.mkdirSync('.cc/temp', {recursive:true});
    fs.writeFileSync(`.cc/temp/native-string-names-${profile}.json`, JSON.stringify(rows,null,2)+'\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`),'utf8')));
  });
}

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-assertion-call-tail/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const profile of ['strict', 'loose']) {
  test(`native assertion call tail (${profile})`, async t => {
    const config = path.join(root, `${profile}.json`);
    const rows = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const offset = marker.index + marker[0].length;
      assert.equal(source[offset], ')', `${marker[1]} selects the actual call`);
      const at = {file: 'cases.ts', ...position(lineStarts(source), offset)};
      const native = await compilerHookOracle(config, {at, inspectDiagnosticCache: true});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.equal(api.kind, 'expression', marker[1]);
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      // These entry/exit events retain actual getter order. Diagnostic codes
      // are source-sorted and establish membership only. The final summary's
      // separate traversal is intentionally excluded from the trace below.
      rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        codes: native.beforeSummary.diagnosticCodes,
        demandFailures: native.beforeSummary.demandFailures,
        acquisitions: native.beforeSummary.displayAcquisitions ?? [],
        displays: native.beforeSummary.displayEvents ?? []});
    }
    assert.equal(rows.length, 28);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-assertion-call-tail-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
    t.diagnostic(`${rows.length} calls matched ordinary native API and maintained baselines.`);
  });
}

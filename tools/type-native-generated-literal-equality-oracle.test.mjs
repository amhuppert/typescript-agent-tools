import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-generated-literal-equality/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const profile of ['strict', 'loose']) {
  test(`native generated literal equality (${profile})`, async () => {
    const config = path.join(root, `${profile}.json`);
    const rows = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file: 'cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const native = await compilerHookOracle(config, {at, inspectDiagnosticCache: true});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      assert.equal(native.errorType, false, marker[1]);
      assert.deepEqual(native.beforeSummary.diagnosticCodes, [], marker[1]);
      assert.deepEqual(native.beforeSummary.demandFailures, [], marker[1]);
      // IDs and comparison are captured in the same native query. Their actual
      // decimal spellings are evidence only, never a product output contract.
      const [left, right, comparison] = JSON.parse(native.summary);
      assert.match(left, /^[1-9][0-9]*$/);
      assert.match(right, /^[1-9][0-9]*$/);
      assert.notEqual(left, right, marker[1]);
      assert.equal(typeof comparison, 'boolean');
      if (marker[1] !== 'adjacentPartition') assert.equal(comparison, false, marker[1]);
      rows.push({name: marker[1], summary: native.summary, identities: [left, right], comparison,
        errorType: native.errorType, codes: native.beforeSummary.diagnosticCodes,
        demandFailures: native.beforeSummary.demandFailures});
    }
    assert.equal(rows.length, 3);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-generated-literal-equality-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

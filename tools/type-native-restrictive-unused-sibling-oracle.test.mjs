import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-restrictive-unused-sibling/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const rows = [];

for (const profile of ['strict', 'nocheck']) {
  test(`native restrictive cache with healthy selected sibling (${profile})`, async () => {
    for (const name of ['query', 'healthy']) {
      const marker = `/*#${name}*/`;
      const at = {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(marker) + marker.length)};
      const config = path.join(root, `${profile}.json`);
      const native = await compilerHookOracle(config, {at, history: 'cold', inspectConditionalCaches: true,
        inspectDiagnosticCache: true, inspectExpressionAcquisitions: true});
      const api = await typeOracle(config, {at, history: 'cold', properties: false});
      const restrictive = Object.entries(native.beforeSummary.observations)
        .filter(([key]) => key.startsWith('restrictive:'))
        .map(([key, value]) => ({key, ...value}));
      const row = {profile, name, at, summary: native.summary, errorType: native.errorType,
        codes: native.beforeSummary.diagnosticCodes, failures: native.beforeSummary.demandFailures,
        restrictive, acquisitions: native.beforeSummary.displayAcquisitions ?? [],
        inputHashes: native.inputHashes};
      rows.push(row);
      fs.writeFileSync(scratchPath('native-restrictive-unused-sibling.json'), JSON.stringify(rows, null, 2) + '\n');
      assert.equal(native.compiler, '7.0.2');
      assert.equal(native.checkerCount, 1);
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType]);
      assert.deepEqual([native.summary, native.errorType], [name === 'query' ? '41' : '45', false]);
      assert.deepEqual(row.failures, []);
      assert.deepEqual(row.codes, name === 'query' ? [2304] : []);
      // MissingMarker is consumed as a symbol during containment. Its sibling
      // is never acquired as an erroneous type, including the final value read.
      assert.deepEqual(row.acquisitions.filter(event => event.event === 'exit' && event.errorType), []);
      if (name === 'query') {
        assert.deepEqual(restrictive.map(event => ({flags: event.flags, hit: event.cacheHit,
          same: event.same, before: event.cacheSelfBefore, after: event.cacheSelfAfter,
          errors: event.errorAcquisitions, diagnostics: event.diagnosticsAdded})), [
          {flags: 524288, hit: false, same: true, before: false, after: true, errors: 0, diagnostics: 0},
          {flags: 1048576, hit: false, same: true, before: false, after: true, errors: 0, diagnostics: 0},
          {flags: 524288, hit: true, same: true, before: true, after: true, errors: 0, diagnostics: 0},
          {flags: 1048576, hit: true, same: true, before: true, after: true, errors: 0, diagnostics: 0},
        ]);
        const installations = restrictive.filter(event => event.same && !event.cacheHit
          && !event.cacheSelfBefore && event.cacheSelfAfter);
        assert.ok(installations.length > 0);
        for (const event of installations) {
          assert.ok(restrictive.some(hit => hit.input === event.input && hit.same
            && hit.cacheHit && hit.cacheSelfBefore && hit.cacheSelfAfter));
        }
      } else assert.deepEqual(restrictive, []);
    }
  });
}

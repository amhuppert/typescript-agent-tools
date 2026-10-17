import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-promise-inventories/', import.meta.url));
for (const [profile, file, count] of [
  ['tsconfig', 'cases.ts', 14], ['loose', 'cases.ts', 14], ['exact', 'cases.ts', 14],
  ['global-absent', 'global.ts', 2], ['global-bad', 'global.ts', 2], ['global-good', 'global.ts', 2],
]) {
  test(`native facts and promise inventory continuation (${profile})`, async () => {
    const source = fs.readFileSync(path.join(root, file), 'utf8');
    const rows = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file, ...position(lineStarts(source), marker.index + marker[0].length)};
      const config = path.join(root, `${profile}.json`);
      const native = await compilerHookOracle(config, {at});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        // Sorted diagnostics are membership evidence. Dependency order follows
        // the native workers; these zero-demand observations add no getters.
        codes: native.beforeSummary.diagnosticCodes,
        demandFailures: native.beforeSummary.demandFailures,
        lastErrorProperty: native.beforeSummary.observations.nonMissingSymbolErrorType ?? null});
    }
    assert.equal(rows.length, count);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-promise-inventories-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

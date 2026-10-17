import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-promised-recovery/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const profile of ['tsconfig', 'loose', 'exact']) {
  test(`native promise adoption and awaited recovery (${profile})`, async () => {
    const rows = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file: 'cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const native = await compilerHookOracle(path.join(root, `${profile}.json`), {at});
      const api = await typeOracle(path.join(root, `${profile}.json`), {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        // These codes are sorted diagnostics, not a trace or cause-order proof.
        // Existing zero-demand getters record established recovery identities;
        // no additional checker getter is called to manufacture an observation.
        codes: native.beforeSummary.diagnosticCodes,
        demandFailures: native.beforeSummary.demandFailures,
        defaultError: native.beforeSummary.observations.typeParameterDefaultError ?? null,
        lastErrorProperty: native.beforeSummary.observations.nonMissingSymbolErrorType ?? null});
    }
    assert.equal(rows.length, 29);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-promised-recovery-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    const expected = path.join(root, `expected-${profile}.json`);
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(expected, 'utf8')));
  });
}

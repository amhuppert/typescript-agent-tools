import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-enum-types/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const markers = [...source.matchAll(/\/\*#([^*]+)\*\//g)];

// diagnosticCodes are source-sorted membership. Acquisition order is derived
// from getDeclaredTypeOfEnum/computeEnumMemberValues, never from that list.
for (const profile of ['tsconfig', 'loose', 'isolated']) {
  test(`native enum values and types (${profile})`, async t => {
    const rows = [];
    for (const marker of markers) {
      const at = {file: 'cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const config = path.join(root, `${profile}.json`);
      const native = await compilerHookOracle(config, {at});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2', marker[1]);
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      rows.push({name: marker[1], kind: api.kind, summary: api.summary, errorType: api.errorType,
        codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures});
    }
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-enum-types-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
    t.diagnostic(`${rows.length} enum roots matched ordinary pinned API; exact maintained baselines asserted.`);
  });
}

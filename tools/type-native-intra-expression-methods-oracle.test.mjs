import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-intra-expression-methods/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const profile of ['strict', 'loose', 'nocheck']) {
  test(`native intra-expression method inference (${profile})`, async () => {
    const rows = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file: 'cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const config = path.join(root, `${profile}.json`);
      const native = await compilerHookOracle(config, {at, history: 'cold'});
      const api = await typeOracle(config, {at, history: 'cold', properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures});
      fs.writeFileSync(`.cc/temp/native-intra-expression-methods-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    }
    assert.equal(rows.length, 11);
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

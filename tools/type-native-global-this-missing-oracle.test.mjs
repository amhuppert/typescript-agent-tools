import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-global-this-missing/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const profile of ['strict', 'loose']) {
  test(`native missing globalThis property (${profile})`, async () => {
    const config = path.join(root, `${profile}.json`);
    const rows = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file: 'cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const native = await compilerHookOracle(config, {at, inspectDiagnosticCache: true});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        codes: native.beforeSummary.diagnosticCodes,
        demandFailures: native.beforeSummary.demandFailures,
        displayEvents: native.beforeSummary.displayEvents ?? []});
    }
    assert.equal(rows.length, 10);
    for (const row of rows) {
      const fallback = !['actualValue', 'actualFunction', 'genuineAny'].includes(row.name);
      const lexical = ['lexicalLet', 'lexicalConst', 'lexicalClass', 'visibleEnum'].includes(row.name);
      assert.equal(row.errorType, false, row.name);
      if (fallback) assert.equal(row.summary, 'any', row.name);
      const tracks = row.displayEvents.filter(event => event.event === 'track');
      assert.equal(tracks.length, fallback && (profile === 'strict' || lexical) ? 2 : 0, row.name);
      assert.ok(tracks.every(event => event.name === 'globalThis'), row.name);
      assert.equal(row.codes.includes(2304), false, row.name);
    }
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-global-this-missing-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

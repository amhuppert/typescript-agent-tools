import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-mapped-limited/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const profile of ['tsconfig', 'loose', 'exact']) {
  test(`native limited mapped inference and excess-property recovery (${profile})`, async () => {
    const rows = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file: 'cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const native = await compilerHookOracle(path.join(root, `${profile}.json`), {at});
      const api = await typeOracle(path.join(root, `${profile}.json`), {at, properties: true});
      assert.equal(native.compiler, '7.0.2');
      assert.equal(api.summary, native.summary, marker[1]);
      rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        codes: native.beforeSummary.diagnosticCodes,
        properties: api.properties.map(({name, summary, errorType}) => ({name, summary, errorType}))});
    }
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-mapped-limited-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    // The rejected source is the independently measured original limited row.
    // Removing only its extra property distinguishes successful inference from
    // resolveCall's overload fallback, despite the same returned type.
    assert.deepEqual(rows, ['healthy', 'rejected'].map(name => ({name,
      summary: '{ a: string; b: number; }', errorType: false,
      codes: name === 'healthy' ? [] : [2353],
      properties: [{name: 'a', summary: 'string', errorType: false}, {name: 'b', summary: 'number', errorType: false}],
    })));
  });
}

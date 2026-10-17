import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-commonjs-type-exports/', import.meta.url));
for (const profile of ['strict', 'loose']) {
  test(`bindCommonJSTypeExports after module children (${profile})`, async () => {
    const rows = [];
    for (const file of ['cases.ts', 'namespace.cts']) {
      const source = fs.readFileSync(path.join(root, file), 'utf8');
      for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
        const at = {file, ...position(lineStarts(source), marker.index + marker[0].length)};
        const config = path.join(root, `${profile}.json`);
        const native = await compilerHookOracle(config, {at});
        const api = await typeOracle(config, {at, properties: false});
        assert.equal(native.compiler, '7.0.2');
        assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
        rows.push({name: marker[1], file, kind: api.kind, summary: native.summary,
          errorType: native.errorType, codes: native.beforeSummary.diagnosticCodes,
          demandFailures: native.beforeSummary.demandFailures});
      }
    }
    assert.equal(rows.length, 16);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-commonjs-type-exports-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    const expected = path.join(root, `expected-${profile}.json`);
    if (!fs.existsSync(expected)) throw new Error(`Captured native rows; install and replay ${expected}`);
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(expected, 'utf8')));
  });
}

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-global-this-raw/', import.meta.url));
test('native initialized globalThis and original declaration symbol identities', async () => {
  const rows = [];
  for (const scenario of ['overload', 'overload_reverse', 'local', 'collision']) {
    const file = `${scenario === 'overload_reverse' ? 'overload' : scenario}.ts`;
    const source = fs.readFileSync(path.join(root, file), 'utf8');
    const config = path.join(root, `${scenario}.json`);
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file, ...position(lineStarts(source), marker.index + marker[0].length)};
      const native = await compilerHookOracle(config, {at});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], `${scenario}/${marker[1]}`);
      const observations = Object.fromEntries(Object.entries(native.beforeSummary.observations)
        .filter(([key]) => key.startsWith('globalThis')));
      rows.push({scenario, name: marker[1], summary: native.summary, errorType: native.errorType,
        codes: native.beforeSummary.diagnosticCodes, observations});
    }
  }
  assert.equal(rows.length, 11);
  fs.mkdirSync('.cc/temp', {recursive: true});
  fs.writeFileSync('.cc/temp/native-global-this-raw.json', JSON.stringify(rows, null, 2) + '\n');
  assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, 'expected.json'), 'utf8')));
});

test('native duplicate globalThis aliases are rejected before queries even with skipLibCheck', async () => {
  const config = path.join(root, 'duplicate.json');
  const source = fs.readFileSync(path.join(root, 'duplicate.ts'), 'utf8');
  const marker = '/*#identifier*/';
  const at = {file: 'duplicate.ts', ...position(lineStarts(source), source.indexOf(marker) + marker.length)};
  for (const oracle of [compilerHookOracle, typeOracle]) {
    await assert.rejects(oracle(config, {at, properties: false}), /TypeScript rejected the project: TS2300 Duplicate identifier 'globalThis'\./);
  }
});

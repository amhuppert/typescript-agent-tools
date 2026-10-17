import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-mixed-alias-variables/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const profile of ['tsconfig', 'loose']) {
  test(`native Alias and Variable flags share one actual header (${profile})`, async () => {
    const rows = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const offset = marker.index + marker[0].length;
      const at = {file: 'cases.ts', ...position(lineStarts(source), offset)};
      const config = path.join(root, `${profile}.json`);
      const native = await compilerHookOracle(config, {at});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      const flows = Object.entries(native.beforeSummary.observations)
        .filter(([name]) => name.startsWith('referenceFlow:'))
        .map(([, flow]) => flow);
      rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        symbolFlags: native.symbolFlags, flows,
        codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures});
    }
    assert.equal(rows.length, 19);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-mixed-alias-variables-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    const expected = path.join(root, `expected-${profile}.json`);
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(expected, 'utf8')));
    for (const row of rows.filter(row => /^(first|last)/.test(row.name))) {
      assert.notEqual(row.symbolFlags & (1 << 21), 0, `${row.name}: actual Alias flag`);
      assert.notEqual(row.symbolFlags & 3, 0, `${row.name}: actual Variable flag`);
      assert.equal(row.summary, 'number');
      assert.equal(row.errorType, false);
    }
  });
}

test('native merged namespace own members precede alias-member fallback', async () => {
  const source = fs.readFileSync(path.join(root, 'namespace-cases.ts'), 'utf8');
  const rows = [];
  for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
    const at = {file: 'namespace-cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
    const config = path.join(root, 'namespaces.json');
    const native = await compilerHookOracle(config, {at});
    const api = await typeOracle(config, {at, properties: false});
    assert.equal(native.compiler, '7.0.2');
    assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
    rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
      codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures});
  }
  assert.equal(rows.length, 5);
  fs.mkdirSync('.cc/temp', {recursive: true});
  fs.writeFileSync('.cc/temp/native-mixed-alias-variables-namespaces.json', JSON.stringify(rows, null, 2) + '\n');
  const expected = path.join(root, 'expected-namespaces.json');
  assert.deepEqual(rows, JSON.parse(fs.readFileSync(expected, 'utf8')));
});

test('native repeated star-module augmentations keep resolved and module export caches separate', async () => {
  const source = fs.readFileSync(path.join(root, 'augmentation-cases.ts'), 'utf8');
  const rows = [];
  for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
    const at = {file: 'augmentation-cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
    const config = path.join(root, 'augmentations.json');
    const native = await compilerHookOracle(config, {at});
    const api = await typeOracle(config, {at, properties: false});
    assert.equal(native.compiler, '7.0.2');
    assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
    rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
      codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures});
  }
  assert.equal(rows.length, 6);
  fs.mkdirSync('.cc/temp', {recursive: true});
  fs.writeFileSync('.cc/temp/native-mixed-alias-variables-augmentations.json', JSON.stringify(rows, null, 2) + '\n');
  const expected = path.join(root, 'expected-augmentations.json');
  assert.deepEqual(rows, JSON.parse(fs.readFileSync(expected, 'utf8')));
});

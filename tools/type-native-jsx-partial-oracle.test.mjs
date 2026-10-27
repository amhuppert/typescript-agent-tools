import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-jsx-partial/', import.meta.url));
const names = ['globals.d.ts', 'model.tsx', 'consumer.ts', 'tsconfig.json'];
const hash = name => crypto.createHash('sha256').update(fs.readFileSync(path.join(root, name))).digest('hex');
const snapshots = names.map(file => ({file: path.join(root, file), sha256: hash(file)}));
const expected = {deferredValue: 'Element', rootModel: 'Model', known: 'string',
  deferred: 'Element', tag: 'number', conditional: 'number'};

test('native healthy JSX model and deferred property for partial expansion', async () => {
  const rows = [];
  const config = path.join(root, 'tsconfig.json');
  for (const file of ['model.tsx', 'consumer.ts']) {
    const source = fs.readFileSync(path.join(root, file), 'utf8');
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const name = marker[1];
      const at = {file, ...position(lineStarts(source), marker.index + marker[0].length)};
      const native = await compilerHookOracle(config, {at, history: 'cold',
        inspectDiagnosticCache: true, inspectExpressionAcquisitions: true});
      const api = await typeOracle(config, {at, history: 'cold', properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.equal(native.checkerCount, 1);
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], name);
      assert.deepEqual([native.summary, native.errorType], [expected[name], false], name);
      assert.deepEqual(native.beforeSummary.diagnosticCodes, [], name);
      assert.deepEqual(native.beforeSummary.demandFailures, [], name);
      assert.deepEqual((native.beforeSummary.displayAcquisitions ?? [])
        .filter(event => event.event === 'exit' && event.errorType), [], name);
      for (const snapshot of snapshots) {
        assert.deepEqual(native.inputHashes.find(input => input.file === snapshot.file), snapshot, name);
      }
      rows.push({name, at, summary: native.summary, errorType: native.errorType,
        codes: native.beforeSummary.diagnosticCodes,
        demandFailures: native.beforeSummary.demandFailures, inputHashes: native.inputHashes});
      fs.writeFileSync(scratchPath('native-jsx-partial.json'), JSON.stringify(rows, null, 2) + '\n');
    }
  }
  assert.deepEqual(rows.map(row => row.name), Object.keys(expected));
  for (const snapshot of snapshots) {
    assert.equal(hash(path.basename(snapshot.file)), snapshot.sha256);
  }
});

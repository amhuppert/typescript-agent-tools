import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-iteration-protocol/', import.meta.url));
for (const [config, file, count] of [
  ['strict', 'cases.ts', 29], ['loose', 'cases.ts', 29], ['exact', 'cases.ts', 29],
  ['absent', 'legacy.ts', 6], ['kind', 'legacy.ts', 6], ['arity', 'legacy.ts', 6],
]) {
  test(`native iterable protocol (${config})`, async () => {
    const source = fs.readFileSync(path.join(root, file), 'utf8');
    const rows = [];
    for (const match of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file, ...position(lineStarts(source), match.index + match[0].length)};
      const nativePanic = {
        asyncOnlyReturn: {description: 'getAsyncFromSyncIterationTypes awaited nil yield', frame: /isAwaitedTypeInstantiation/},
        asyncBadThen: {description: 'getAsyncFromSyncIterationTypes invalid thenable nil diagnostic', frame: /NewDiagnosticChainForNode/},
      }[match[1]];
      if (nativePanic) {
        let hookFailure;
        await assert.rejects(() => compilerHookOracle(path.join(root, `${config}.json`), {at}), error => {
          assert.match(error.message, /panic: runtime error: invalid memory address or nil pointer dereference/);
          assert.match(error.message, nativePanic.frame);
          assert.match(error.message, /getAsyncFromSyncIterationTypes/);
          hookFailure = error.message;
          return true;
        });
        let apiFailure;
        await assert.rejects(() => typeOracle(path.join(root, `${config}.json`), {at, properties: false}), error => {
          assert.ok(error instanceof Error);
          assert.match(error.message, /panic: runtime error: invalid memory address or nil pointer dereference/);
          assert.match(error.message, nativePanic.frame);
          assert.match(error.message, /getAsyncFromSyncIterationTypes/);
          apiFailure = {name: error.name, message: error.message};
          return true;
        });
        fs.mkdirSync('.cc/temp', {recursive: true});
        fs.writeFileSync(`.cc/temp/native-iteration-${config}-${match[1]}-panic.json`, JSON.stringify({at, hookFailure, apiFailure}, null, 2) + '\n');
        const apiFrames = [...apiFailure.message.matchAll(/^github\.com\/microsoft\/typescript-go\/internal\/checker\.\(\*Checker\)\.([^\n(]+)[^\n]*\n\s+github\.com\/microsoft\/typescript-go\/internal\/checker\/checker\.go:(\d+)/gm)]
          .map(([,name,line]) => `${name}:${line}`);
        assert.ok(apiFrames.length > 0);
        rows.push({name: match[1], nativePanic: nativePanic.description, apiFrames});
        continue;
      }
      const native = await compilerHookOracle(path.join(root, `${config}.json`), {at});
      const api = await typeOracle(path.join(root, `${config}.json`), {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], match[1]);
      const observations = native.beforeSummary.observations;
      rows.push({name: match[1], kind: api.kind, summary: api.summary, errorType: api.errorType,
        codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures,
        iteration: Object.fromEntries(Object.entries(observations).filter(([key]) =>
          key.startsWith('checkedIteration:') || key.startsWith('iteratorMethod:') || key.startsWith('iteratorMissingValue:') || key.startsWith('iterationAwaitedRecovery:') || key === 'asyncFromSyncIteration'))});
    }
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-iteration-${config}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.equal(rows.length, count);
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${config}.json`), 'utf8')));
  });
}

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-flow-continuation/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const dependencies = [...source.matchAll(/\/\*@([^*]+)\*\//g)].map(marker => ({name: marker[1], end: marker.index + marker[0].length + 1}));
for (const profile of ['tsconfig', 'loose']) {
  test(`native flow continuation (${profile})`, async () => {
    const config = path.join(root, `${profile}.json`);
    const rows = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const offset = marker.index + marker[0].length;
      const at = {file: 'cases.ts', ...position(lineStarts(source), offset)};
      const native = await compilerHookOracle(config, {at});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      const flows = Object.entries(native.beforeSummary.observations)
        .filter(([key]) => key.startsWith('referenceFlow:')).map(([, value]) => value);
      const selected = flows.find(flow => flow.referenceEnd === offset + 1);
      assert.ok(selected, `${marker[1]} selected native flow return`);
      rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        sameDeclared: selected.sameDeclared, sameInitial: selected.sameInitial,
        acquired: dependencies.filter(dependency => flows.some(flow => flow.referenceEnd === dependency.end && flow.errorType)).map(dependency => dependency.name),
        codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures});
    }
    assert.equal(rows.length, 16);
    if (profile === 'tsconfig') {
      assert.equal(rows.find(row => row.name === 'optionalSatisfiedEquality').sameDeclared, false);
      assert.equal(rows.find(row => row.name === 'optionalAssignedEquality').sameDeclared, true);
    }
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-flow-continuation-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

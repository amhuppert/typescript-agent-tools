import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-alias-conditions/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const tokenEnd = (text, offset) => {
  const token = /^[A-Za-z_$][A-Za-z0-9_$]*/.exec(text.slice(offset));
  assert.ok(token, 'marker must precede an exact identifier');
  return offset + token[0].length;
};
const dependencies = ['cases.ts', 'conditions.ts'].flatMap(file => {
  const text = fs.readFileSync(path.join(root, file), 'utf8');
  return [...text.matchAll(/\/\*@([^*]+)\*\//g)].map(marker => ({
    name: marker[1], file, end: tokenEnd(text, marker.index + marker[0].length),
  }));
});
// The existing flow observer keys by native offsets, not source file. Keep
// these deliberately tiny sources collision-free rather than claim that the
// observer supplies source identity or acquisition order it does not retain.
const selectedEnds = [...source.matchAll(/\/\*#([^*]+)\*\//g)]
  .map(marker => tokenEnd(source, marker.index + marker[0].length));
const observedEnds = [...selectedEnds, ...dependencies.map(dependency => dependency.end)];
assert.equal(new Set(observedEnds).size, observedEnds.length);

for (const profile of ['tsconfig', 'loose']) {
  test(`native alias condition inlining (${profile})`, async () => {
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
      const selected = flows.find(flow => flow.referenceEnd === tokenEnd(source, offset));
      assert.ok(selected, `${marker[1]} selected native flow return`);
      rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        sameDeclared: selected.sameDeclared, sameInitial: selected.sameInitial,
        acquired: dependencies.filter(dependency => flows.some(flow => flow.referenceEnd === dependency.end && flow.errorType))
          .map(({name, file}) => ({name, file})),
        codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures});
    }
    assert.equal(rows.length, 34);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-alias-conditions-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-inference-boundaries/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'sites.ts'), 'utf8');
const cases = [
  "indexNumeric",
  "indexNumericString",
  "indexComputed",
  "indexInstantiated",
  "indexOptional",
  "indexString",
  "indexPublicClass",
  "indexPrivateClass",
  "varianceCovariant",
  "varianceContravariant",
  "varianceInvariant",
  "varianceMethod",
  "varianceIndependent",
  "varianceRecursive",
  "varianceDeclaredOut",
  "varianceDeclaredIn",
  "varianceDeclaredInvariant",
  "varianceFailure",
  "late",
  "lateFailure"
];
function expected(name, profile) {
  if (name === 'late') return '{ value: number; }';
  if (name === 'lateFailure') return '{ value: MissingLateDefault; }';
  if (name === 'indexPublicClass' || name === 'indexPrivateClass') return '"same"';
  if (name === 'indexOptional' && !['exact.json', 'loose.json'].includes(profile)) return '"same"';
  if (name === 'varianceDeclaredIn' || name === 'varianceDeclaredInvariant') return '"same"';
  if (['varianceContravariant', 'varianceInvariant'].includes(name) && profile !== 'bivariant.json') return '"same"';
  return '"chosen"';
}

for (const profile of ['tsconfig.json', 'loose.json', 'exact.json', 'bivariant.json']) {
  test(`native index names, variance and late inference slots: ${profile}`, async t => {
    const observations = [];
    for (const name of cases) {
      const offset = source.indexOf(`${name}Result =`);
      assert.ok(offset >= 0, name);
      const at = {file: 'sites.ts', ...position(lineStarts(source), offset)};
      const config = path.join(root, profile);
      const answer = await typeOracle(config, {at, properties: false});
      const observed = await compilerHookOracle(config, {operation: 'type', at});
      observations.push({name, summary: answer.summary, errorType: answer.errorType,
        diagnosticCodes: observed.beforeSummary.diagnosticCodes,
        demandFailures: observed.beforeSummary.demandFailures});
      assert.equal(answer.compiler, '7.0.2', name);
      assert.equal(observed.summary, answer.summary, name);
      assert.equal(observed.errorType, answer.errorType, name);
    }
    t.diagnostic(JSON.stringify(observations));
    for (const observed of observations) {
      assert.equal(observed.summary, expected(observed.name, profile), observed.name);
      assert.equal(observed.errorType, false, observed.name);
      assert.deepEqual(observed.demandFailures, [], observed.name);
      assert.equal(observed.diagnosticCodes.filter(code => code === 2304 || code === 2552).length,
        ['varianceFailure', 'lateFailure'].includes(observed.name) ? 1 : 0,
        `${observed.name}: only an actually consumed missing input is diagnosed`);
      assert.ok(!observed.diagnosticCodes.includes(2345), observed.name);
    }
  });
}

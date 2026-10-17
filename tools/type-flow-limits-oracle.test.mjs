import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { test } from 'node:test';
import { compilerHookOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

function sourceFor(branches, inside = false, assignments = 0) {
  return 'export function flowLimit(): void {\nlet other: any;\nconst value: number = 1;\n'
    + 'if (other) {}\n'.repeat(branches) + 'other = 1;\n'.repeat(assignments)
    + (inside ? 'if (other) { /*#query*/value; }\n' : '/*#query*/value;\n') + '}\n';
}

async function query(t, source, strict = true) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-flow-depth-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  fs.writeFileSync(path.join(directory, 'flow.ts'), source);
  fs.writeFileSync(path.join(directory, 'tsconfig.json'), JSON.stringify({ compilerOptions: { strict, noLib: true }, files: ['flow.ts'] }));
  const offset = source.indexOf('/*#query*/') + '/*#query*/'.length;
  return compilerHookOracle(path.join(directory, 'tsconfig.json'), {
    at: { file: 'flow.ts', ...position(lineStarts(source), offset) }, properties: false,
  });
}

test('native flow-depth boundary is recursive dependency depth', async t => {
  for (const [branches, inside, depth, failed] of [[999, true, 1999, false], [1000, false, 2000, true], [1001, false, 2000, true]]) {
    const answer = await query(t, sourceFor(branches, inside));
    assert.equal(answer.beforeSummary.counters.flowDepth, depth, `${branches}/${inside}`);
    assert.equal(answer.errorType, failed, `${branches}/${inside}`);
    assert.equal(answer.summary, failed ? 'any' : 'number');
    assert.deepEqual(answer.beforeSummary.demandFailures, failed ? ['error_type'] : []);
    assert.deepEqual(answer.beforeSummary.diagnosticCodes.filter(code => code !== 2318), failed ? [2563, 2563] : [], `${branches}/${inside}`);
  }
});

test('native long flat assignments stay at the iterative flow depth', async t => {
  const answer = await query(t, sourceFor(0, false, 4000));
  assert.equal(answer.summary, 'number');
  assert.equal(answer.errorType, false);
  assert.equal(answer.beforeSummary.counters.flowDepth, 0);
  assert.deepEqual(answer.beforeSummary.demandFailures, []);
  assert.deepEqual(answer.beforeSummary.diagnosticCodes.filter(code => code !== 2318), []);
});

test('native flow-depth admission has the same boundary in loose mode', async t => {
  for (const [branches, inside, depth, failed, assignments] of [[999, true, 1999, false, 0], [1000, false, 2000, true, 0], [1001, false, 2000, true, 0], [0, false, 0, false, 4000]]) {
    const answer = await query(t, sourceFor(branches, inside, assignments), false);
    assert.equal(answer.beforeSummary.counters.flowDepth, depth, `${branches}/${inside}`);
    assert.equal(answer.errorType, failed, `${branches}/${inside}`);
    assert.equal(answer.summary, failed ? 'any' : 'number');
    assert.deepEqual(answer.beforeSummary.demandFailures, failed ? ['error_type'] : []);
    assert.deepEqual(answer.beforeSummary.diagnosticCodes.filter(code => code !== 2318), failed ? [2563, 2563] : [], `${branches}/${inside}`);
  }
});


// Shallow syntax with one surviving false-condition antecedent at each step.
// Unlike empty-if diamonds, this path does not repeatedly consume Shared
// recovery completions, so the native depth producer is isolated from the
// product's exact omitted-cause-count output limit.
function linearSource(guards) {
  return 'export function flowLimit(): void {\nlet other: any;\nconst value: number = 1;\n'
    + 'if (other) throw 0;\n'.repeat(guards) + '/*#query*/value;\n}\n';
}

test('native linear flow isolates just-under, at and over recursive depth 2000', async t => {
  const rows = [];
  for (const strict of [true, false]) for (const guards of [1999, 2000, 2001]) {
    const answer = await query(t, linearSource(guards), strict);
    rows.push({strict, guards, depth: answer.beforeSummary.counters.flowDepth,
      summary: answer.summary, errorType: answer.errorType,
      failures: answer.beforeSummary.demandFailures,
      codes: answer.beforeSummary.diagnosticCodes.filter(code => code !== 2318)});
  }
  fs.mkdirSync('.cc/temp', {recursive: true});
  fs.writeFileSync('.cc/temp/native-linear-flow-depth.json', JSON.stringify(rows, null, 2) + '\n');
  for (const row of rows) {
    assert.equal(row.depth, Math.min(row.guards, 2000), JSON.stringify(row));
    assert.equal(row.errorType, row.guards >= 2000, JSON.stringify(row));
    assert.equal(row.summary, row.guards >= 2000 ? 'any' : 'number');
    assert.deepEqual(row.failures, row.guards >= 2000 ? ['error_type'] : []);
    assert.deepEqual(row.codes, row.guards >= 2000 ? [2563] : []);
  }
});

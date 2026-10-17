import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-assignment-continuation/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const markers = [...source.matchAll(/\/\*#([^*]+)\*\//g)];

function assignmentEvents(observations) {
  const ids = new Map();
  const identity = id => {
    if (!ids.has(id)) ids.set(id, ids.size + 1);
    return ids.get(id);
  };
  const rows = Object.entries(observations).filter(([key]) => key.startsWith('assignment:'))
    .sort(([a], [b]) => Number(a.slice(11)) - Number(b.slice(11)))
    .map(([, event]) => ({...event}));
  const installed = new Map();
  for (const event of rows) {
    if (event.declared !== undefined) {
      event.declared = identity(event.declared);
      event.assigned = identity(event.assigned);
      const key = `${event.declared}:${event.assigned}`;
      if (event.event === 'lookup' && event.hit) {
        assert.ok(installed.has(key), 'within-query cache hit must follow an actual publication');
      } else if (event.event === 'installed') {
        installed.set(key, true);
      }
    }
  }
  return rows;
}

for (const profile of ['tsconfig', 'loose', 'unchecked']) {
  test(`native assignment continuation and cache (${profile})`, async t => {
    const rows = [];
    for (const marker of markers) {
      const at = {file: 'cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const config = path.join(root, `${profile}.json`);
      const native = await compilerHookOracle(config, {at});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2', marker[1]);
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      rows.push({name: marker[1], kind: api.kind, summary: api.summary, errorType: api.errorType,
        codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures,
        assignments: assignmentEvents(native.beforeSummary.observations)});
    }
    assert.equal(rows.length, 29);
    for (const name of ['reductionWarm', 'reductionWarmReverse', 'healthyReductionWarm']) {
      const events = rows.find(row => row.name === name).assignments;
      assert.deepEqual(events.map(event => [event.event, event.hit ?? null]),
        [['lookup', false], ['installed', null], ['lookup', true]], name);
      assert.equal(events[0].declared, events[2].declared, name);
      assert.equal(events[0].assigned, events[2].assigned, name);
    }
    for (const name of ['assignedIdentity', 'assignedNever', 'scalarSkipsAssignment']) {
      assert.deepEqual(rows.find(row => row.name === name).assignments, [], name);
    }
    for (const name of ['propertyRecovery', 'indexRecovery', 'absentRecovery',
      'unusableKeyRecovery', 'shorthandRecovery', 'tupleRecovery', 'tupleOutOfBounds',
      'iterationRecovery', 'bindingInitialRecovery']) {
      const events = rows.find(row => row.name === name).assignments;
      assert.deepEqual(events.slice(0, 2).map(event => event.event),
        ['defaultEnter', 'defaultExit'], name);
      assert.equal(events[1].errorType, true, name);
      assert.equal(events.at(-1).event, 'installed', name);
    }
    // The nested array is not isDestructuringAssignmentTarget: native treats
    // its binary expression as ordinary RHS, so only later.value is acquired.
    assert.equal(rows.find(row => row.name === 'nestedRecovery').assignments
      .some(event => event.event === 'defaultEnter'), false);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-assignment-continuation-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
    t.diagnostic(`${rows.length} assignment roots matched the ordinary pinned API and exact maintained baselines.`);
  });
}

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-diagnostic-cache/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const profile of ['tsconfig', 'loose', 'untruncated']) {
  test(`native diagnostic node-cache acquisition (${profile})`, async () => {
    const rows = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file: 'cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const config = path.join(root, `${profile}.json`);
      const native = await compilerHookOracle(config, {at, inspectDiagnosticCache: true});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      assertCacheTrace(native.beforeSummary.displayEvents ?? [], marker[1]);
      rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        beforeCodes: native.beforeSummary.diagnosticCodes,
        afterCodes: native.diagnosticCodes,
        beforeEvents: native.beforeSummary.displayEvents ?? [], events: native.displayEvents ?? []});
    }
    assert.equal(rows.length, 14);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-diagnostic-cache-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
  });
}

// Check actual native transitions, not diagnostic ordering or guessed IDs.
// All IDs are compared only within this single checker invocation.
function assertCacheTrace(events, name) {
  const active = [];
  const installed = new Map();
  const key = e => [e.scope, e.type, e.flags, e.internalFlags].join(':');
  for (const event of events) {
    if (event.event === 'track') {
      if (active.at(-1)?.hit) active.at(-1).replayedTracks++;
      continue;
    }
    assert.equal(event.expansion, -1, name);
    assert.equal(event.internalFlags, 0, name);
    if (event.event === 'visit') {
      active.push({visit: event, replayedTracks: 0});
      continue;
    }
    const frame = active.at(-1);
    assert.ok(frame, `${name}: ${event.event} without active transform`);
    assert.equal(event.type, frame.visit.type, name);
    assert.equal(event.flags, frame.visit.flags, name);
    switch (event.event) {
      case 'miss':
        frame.miss = true;
        break;
      case 'hit': {
        assert.notEqual(event.scope, 0, `${name}: native never reads nil scope`);
        const previous = installed.get(key(event));
        assert.ok(previous, `${name}: actual key must have been installed`);
        assert.deepEqual([event.addedLength, event.cachedTruncating, event.cachedTracked],
          [previous.addedLength, previous.cachedTruncating, previous.cachedTracked], name);
        assert.equal(event.visited, frame.visit.visited, name);
        frame.hit = event;
        break;
      }
      case 'replayed':
        assert.ok(frame.hit, name);
        assert.equal(event.length, frame.hit.length + frame.hit.addedLength, name);
        assert.equal(event.truncating, frame.hit.truncating || frame.hit.cachedTruncating, name);
        assert.equal(frame.replayedTracks, frame.hit.cachedTracked, name);
        assert.equal(event.tracked, frame.hit.tracked + frame.hit.cachedTracked, name);
        active.pop();
        break;
      case 'transformed':
        assert.ok(frame.miss, name);
        assert.equal(event.visited, true, name);
        frame.transformed = true;
        break;
      case 'installed':
        assert.ok(frame.transformed, name);
        assert.equal(event.reportedDiagnostic, false, name);
        assert.equal(event.encounteredError, false, name);
        assert.equal(event.addedLength, event.length - frame.visit.length, name);
        assert.equal(event.cachedTracked, event.tracked, name);
        installed.set(key(event), event);
        frame.finished = true;
        break;
      case 'notInstalled':
        assert.ok(frame.transformed, name);
        assert.ok(event.reportedDiagnostic || event.encounteredError, name);
        frame.finished = true;
        break;
      case 'cleaned':
        assert.ok(frame.finished, name);
        assert.equal(event.visited, false, name);
        assert.equal(event.tracked, frame.visit.tracked, name);
        active.pop();
        break;
      case 'depthElision':
        assert.ok(frame.miss && event.depth > 10, name);
        active.pop();
        break;
      default: assert.fail(`${name}: unknown cache event ${event.event}`);
    }
  }
  assert.equal(active.length, 0, name);
}

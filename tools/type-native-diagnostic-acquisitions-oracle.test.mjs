import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-diagnostic-cache/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const profile of ['tsconfig', 'loose', 'untruncated']) {
  test(`native diagnostic recovery acquisitions (${profile})`, async () => {
    const rows = [];
    for (const name of ['recoveredCold', 'recoveredWarm']) {
      const marker = `/*#${name}*/`;
      const at = {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(marker) + marker.length)};
      const config = path.join(root, `${profile}.json`);
      const native = await compilerHookOracle(config, {at, inspectDiagnosticCache: true});
      const api = await typeOracle(config, {at, properties: false});
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], name);
      const events = native.beforeSummary.displayAcquisitions;
      const stack = [];
      for (const event of events) {
        if (event.event === 'enter') {
          assert.equal(event.depth, stack.length, name);
          stack.push(event);
        } else {
          const before = stack.pop();
          assert.ok(before, name);
          assert.deepEqual([event.operation,event.depth,event.pos,event.end,event.name],
            [before.operation,before.depth,before.pos,before.end,before.name], name);
        }
      }
      assert.equal(stack.length, 0, name);
      // These are actual getter exits, not source-sorted diagnostic codes.
      // The nested annotation belongs to the first symbol acquisition; the
      // remaining five depth-zero exits are independent consumed completions.
      const failures = events.filter(event => event.event === 'exit' && event.errorType);
      assert.deepEqual(failures.map(event => [event.operation, event.depth]), [
        ['getTypeFromTypeNode', 1], ['getTypeOfSymbol', 0],
        ['getTypeOfSymbol', 0], ['getTypeFromTypeNode', 0],
        ['getTypeFromTypeNode', 0], ['getTypeFromTypeNode', 0],
      ], name);
      const missing = source.indexOf('MissingCache');
      for (const event of failures) {
        if (event.operation === 'getTypeOfSymbol') assert.equal(event.name, 'hidden', name);
        else {
          assert.ok(event.pos <= missing, name); // native pos includes leading trivia
          assert.equal(event.end, missing + 'MissingCache'.length, name);
        }
      }
      const cacheEvents = native.beforeSummary.displayEvents;
      const first = cacheEvents[0];
      assert.equal(first.event, 'visit', name);
      assert.notEqual(first.scope, 0, name);
      const rootEvents = cacheEvents.filter(event => event.type === first.type
        && event.scope === first.scope && event.flags === first.flags
        && event.internalFlags === first.internalFlags);
      const repeats = name === 'recoveredWarm' ? 3 : 1;
      assert.deepEqual(rootEvents.map(event => event.event), [
        'visit', 'miss', 'transformed', 'installed', 'cleaned',
        ...Array.from({length: repeats}, () => ['visit', 'hit', 'replayed']).flat(),
      ], name);
      const installedAt = cacheEvents.findIndex(event => event === rootEvents[3]);
      assert.ok(failures.every(event => event.cacheEvents <= installedAt), name);
      // Warm reads reuse the installed display. The independent relation-cache
      // replay accounting is documented in types-diagnostic-cache-evidence.md.
      rows.push({name, summary:native.summary, codes:native.beforeSummary.diagnosticCodes,
        events, cacheEvents:native.beforeSummary.displayEvents});
    }
    fs.writeFileSync(scratchPath(`native-diagnostic-acquisitions-${profile}.json`), JSON.stringify(rows,null,2)+'\n');
  });
}

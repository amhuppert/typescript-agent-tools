import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-overload-failure-signatures/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const markers = [...source.matchAll(/\/\*#([^*]+)\*\//g)];

// Getter observations prove acquisition membership, not order: the native
// observation map retains one result per actual declaration. Factory order is
// derived separately from createUnionOfSignaturesForOverloadFailure9557.
// Neither observer requests extra signature/type reads.
for (const profile of ['tsconfig', 'loose', 'exact']) {
  test(`native overload-failure signature factory (${profile})`, async t => {
    const rows = [];
    for (const marker of markers) {
      const at = {file: 'cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const config = path.join(root, `${profile}.json`);
      const native = await compilerHookOracle(config, {at});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2', marker[1]);
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      const returns = Object.entries(native.beforeSummary.observations)
        .filter(([key]) => key.startsWith('signatureReturn:'))
        .map(([, value]) => ({
          ...value,
          declaration: source.slice(value.pos, value.end),
        })).sort((a, b) => a.pos - b.pos || a.end - b.end || a.kind - b.kind);
      rows.push({name: marker[1], kind: api.kind, summary: api.summary, errorType: api.errorType,
        codes: native.beforeSummary.diagnosticCodes,
        demandFailures: native.beforeSummary.demandFailures,
        returns});
    }
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-overload-failure-signatures-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.equal(rows.length, 20);
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
    const selected = rows.find(row => row.name === 'selectedSkipsReturn');
    assert.equal(selected.returns.length, 1);
    assert.ok(!selected.returns.some(row => row.declaration.includes('MissingUnchosenReturn')));
    const returns = rows.find(row => row.name === 'allMissingReturns');
    assert.equal(returns.returns.length, 2);
    assert.ok(returns.returns.every(row => row.errorType));
    assert.deepEqual(rows.find(row => row.name === 'genuineAnyReturn').demandFailures, []);
    t.diagnostic('Pinned 20 final queries, exact diagnostic membership and naturally acquired signature returns.');
  });
}

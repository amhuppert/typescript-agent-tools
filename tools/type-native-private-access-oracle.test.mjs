import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-private-access/', import.meta.url));
const profiles = [
  ['tsconfig', 'cases.ts', 26], ['verbatim', 'cases.ts', 26],
  ['diagnostics', 'diagnostics.ts', 14],
  ...['valid', 'short', 'recover', 'alias', 'variables', 'empty', 'missing', 'disabled', 'native', 'assign']
    .map(name => [`helpers-${name}`, 'helpers.ts', 5]),
];
for (const [profile, file, count] of profiles) {
  test(`native private property access (${profile})`, async () => {
    const source = fs.readFileSync(path.join(root, file), 'utf8');
    const config = path.join(root, `${profile}.json`);
    const rows = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file, ...position(lineStarts(source), marker.index + marker[0].length)};
      const native = await compilerHookOracle(config, {at});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      const access = Object.entries(native.beforeSummary.observations)
        .filter(([key]) => key.startsWith('privateAccess:'))
        .sort(([left], [right]) => Number(left.slice(14)) - Number(right.slice(14)))
        .map(([, value]) => value);
      rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        codes: native.beforeSummary.diagnosticCodes,
        demandFailures: native.beforeSummary.demandFailures, access});
    }
    assert.equal(rows.length, count);
    const named = name => rows.find(row => row.name === name);
    const helpers = name => named(name).access.filter(event => event.event === 'helpers').map(event => event.helpers);
    if (profile === 'tsconfig' || profile === 'verbatim') {
      const alias = named('aliasSelf').access;
      assert.ok(alias.some(event => event.event === 'linked' && event.enabled === (profile === 'tsconfig')));
      assert.equal(alias.some(event => event.event === 'alias'), profile === 'tsconfig');
      assert.ok(named('staticName').access.some(event => event.event === 'self' && event.parent && event.result));
      assert.ok(named('qualifiedSelf').access.some(event => event.event === 'self' && event.parent && !event.result));
      assert.deepEqual(named('outerLexical').access.filter(event => event.event === 'members' || event.event === 'exports')
        .slice(0, 3).map(event => [event.event, event.found]), [['members', false], ['exports', false], ['members', true]]);
      assert.equal(named('genuineAny').access.some(event => event.event === 'linked'), false);
    }
    if (profile.startsWith('helpers-')) {
      if (profile === 'helpers-native') assert.deepEqual(helpers('setThenGet'), []);
      else assert.deepEqual(helpers('setThenGet').slice(0, 2), [16384, 8192]);
      if (profile === 'helpers-variables') assert.ok(rows.every(row => !row.codes.includes(2304)));
      if (profile === 'helpers-recover' || profile === 'helpers-assign') {
        assert.equal(named('repeatedGet').codes.filter(code => code === 2304).length, 1);
        assert.equal(named('writeThenRead').codes.filter(code => code === 2304).length, 2);
      }
    }
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-private-access-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

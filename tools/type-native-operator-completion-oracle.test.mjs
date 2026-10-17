import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-operator-completion/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');

for (const profile of ['strict', 'loose', 'nocheck']) {
  test(`native unary and binary operator continuation (${profile})`, async () => {
    const config = path.join(root, `${profile}.json`);
    const queries = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file: 'cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const native = await compilerHookOracle(config, {at, history: 'cold',
        inspectDiagnosticCache: true, inspectExpressionAcquisitions: true});
      const api = await typeOracle(config, {at, history: 'cold', properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      const errors = (native.beforeSummary.displayAcquisitions ?? [])
        .filter(event => event.event === 'exit' && event.errorType)
        .map(({operation, pos, end, declarationPos, declarationEnd, name}) =>
          ({operation, pos, end, declarationPos, declarationEnd, name}));
      queries.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures, errors});
      fs.writeFileSync(`.cc/temp/native-operator-completion-${profile}.json`, JSON.stringify({queries}, null, 2) + '\n');
    }
    assert.equal(queries.length, 16);
    assert.deepEqual(JSON.parse(JSON.stringify({queries})), JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

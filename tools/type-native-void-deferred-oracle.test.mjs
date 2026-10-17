import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-void-deferred/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const expected = {
  skipped: ['65', false, []],
  later: ['MissingVoidLater', true, ['MissingVoidLater']],
  direct: ['undefined', false, []],
  eager: ['65', false, ['MissingVoidFirst']],
};
for (const profile of ['strict', 'loose', 'nocheck']) {
  test(`native Void defers its operand while comma checks eager operands (${profile})`, async () => {
    const rows = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const config = path.join(root, `${profile}.json`);
      const at = {file: 'cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const native = await compilerHookOracle(config, {at, history: 'cold',
        inspectDiagnosticCache: true, inspectExpressionAcquisitions: true});
      const api = await typeOracle(config, {at, history: 'cold', properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.equal(native.checkerCount, 1);
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      const errors = (native.beforeSummary.displayAcquisitions ?? [])
        .filter(event => event.event === 'exit' && event.errorType && event.operation === 'getTypeFromTypeNode');
      const [capturedSummary, errorType, required] = expected[marker[1]];
      const summary = profile === 'loose' && marker[1] === 'direct' ? 'any' : capturedSummary;
      assert.deepEqual([native.summary, native.errorType], [summary, errorType], marker[1]);
      assert.deepEqual(errors.map(event => source.slice(event.pos, event.end).trim()), required, marker[1]);
      assert.ok(errors.every(event => event.operation === 'getTypeFromTypeNode'), marker[1]);
      rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures,
        errors: errors.map(({operation, pos, end}) => ({operation, pos, end}))});
      fs.writeFileSync(`.cc/temp/native-void-deferred-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    }
    assert.equal(rows.length, 4);
  });
}

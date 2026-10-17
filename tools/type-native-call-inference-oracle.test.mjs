import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-call-inference/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'sites.ts'), 'utf8');
test('native ordinary call inference preserves context, defaults, receiver and rejection ordering', async t => {
  const observations = [];
  for (const name of ['identity', 'pair', 'context', 'rejected', 'eagerDefault', 'skippedDefault', 'receiver', 'unused']) {
    const offset = source.indexOf(`${name}Result =`);
    assert.ok(offset >= 0, name);
    const at = {file: 'sites.ts', ...position(lineStarts(source), offset)};
    const config = path.join(root, 'tsconfig.json');
    const answer = await typeOracle(config, {at, properties: name === 'receiver'});
    const observed = await compilerHookOracle(config, {operation: 'type', at});
    observations.push({name, summary:answer.summary, errorType:answer.errorType, diagnostics:observed.beforeSummary.diagnosticCodes, failures:observed.beforeSummary.demandFailures});
    assert.equal(answer.compiler, '7.0.2');
    assert.equal(observed.summary, answer.summary, name);
    assert.equal(observed.errorType, answer.errorType, name);
  }
  t.diagnostic(JSON.stringify(observations));
  const summaries = {identity:'"literal"',pair:'"a" | "b"',context:'"context"',rejected:'"number"',eagerDefault:'1',skippedDefault:'"skipped"',receiver:'Receiver',unused:'unknown'};
  for (const observed of observations) {
    assert.equal(observed.summary, summaries[observed.name], observed.name);
    assert.equal(observed.errorType, false, observed.name);
    assert.deepEqual(observed.failures, [], observed.name);
    assert.equal(observed.diagnostics.filter(code => code === 2304 || code === 2552).length, observed.name === 'eagerDefault' ? 1 : 0, observed.name);
  }
});

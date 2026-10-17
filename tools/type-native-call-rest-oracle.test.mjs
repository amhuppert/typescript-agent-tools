import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-call-rest/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'sites.ts'), 'utf8');
const expectations = JSON.parse(fs.readFileSync(path.join(root, 'expected.json'), 'utf8'));
test('native effective tuple spreads, rest inference and argument scheduling', async t => {
  const observations = [];
  for (const config of ['tsconfig.json', 'loose.json', 'exact.json']) {
    for (const name of Object.keys(expectations)) {
      const at = {file: 'sites.ts', ...position(lineStarts(source), source.indexOf(`${name}Result =`))};
      const answer = await typeOracle(path.join(root, config), {at});
      const hook = await compilerHookOracle(path.join(root, config), {operation:'type', at});
      observations.push({config,name,summary:answer.summary,error:answer.errorType,codes:hook.beforeSummary.diagnosticCodes,failures:hook.beforeSummary.demandFailures});
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.summary, hook.summary, `${config}/${name}/hook`);
    }
  }
  t.diagnostic(JSON.stringify(observations));
  for (const value of observations) {
    const expected = expectations[value.name];
    assert.equal(value.summary, typeof expected === 'string' ? expected : expected[value.config], `${value.config}/${value.name}`);
    assert.equal(value.error,false,`${value.config}/${value.name}`);
    assert.deepEqual(value.failures,[],`${value.config}/${value.name}`);
    assert.equal(value.codes.filter(code=>code===2304||code===2552).length, ['badSpread', 'aritySkipped'].includes(value.name) ? 1 : 0, `${value.config}/${value.name}/dependencies`);
  }
});

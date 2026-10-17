import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-template-types/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const expected = JSON.parse(fs.readFileSync(path.join(root, 'expected.json'), 'utf8'));
const diagnostics = {"missing": [2304], "checked": [1360], "checkedError": [1360, 2304], "missingFirst": [2304], "neverMissing": [2304]};

for (const config of ['tsconfig.json', 'loose.json', 'exact.json']) {
  test(`native template type construction (${config})`, async () => {
    const observed = [];
    const effects = [];
    for (const [, marker] of source.matchAll(/\/\*#(\w+)\*\//g)) {
      const tag = `/*#${marker}*/`;
      const at = { file: 'cases.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length) };
      const answer = await typeOracle(path.join(root, config), { at, properties: false });
      const hook = await compilerHookOracle(path.join(root, config), { operation: 'type', at });
      observed.push({ marker, summary: answer.summary, errorType: answer.errorType });
      assert.equal(answer.summary, hook.summary, marker);
      assert.equal(answer.compiler, '7.0.2');
      assert.deepEqual(hook.beforeSummary.diagnosticCodes, diagnostics[marker] ?? [], marker);
      assert.deepEqual(hook.beforeSummary.demandFailures, [], marker);
      effects.push({ config, marker, codes: hook.beforeSummary.diagnosticCodes, failures: hook.beforeSummary.demandFailures });
    }
    assert.deepEqual(observed, expected[config]);
    fs.mkdirSync('.cc/temp', { recursive: true });
    fs.writeFileSync(`.cc/temp/native-template-types-${config}`, JSON.stringify(observed, null, 2));
    fs.writeFileSync(`.cc/temp/native-template-effects-${config}`, JSON.stringify(effects, null, 2));
  });
}

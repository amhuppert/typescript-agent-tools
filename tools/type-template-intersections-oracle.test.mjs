import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-template-intersections/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const expected = JSON.parse(fs.readFileSync(path.join(root, 'expected.json'), 'utf8'));
for (const config of ['tsconfig.json', 'loose.json', 'exact.json']) {
  test(`native template intersection and mapping identity (${config})`, async () => {
    const observed = [];
    for (const [, marker] of source.matchAll(/\/\*#(\w+)\*\//g)) {
      const tag = `/*#${marker}*/`;
      const at = { file: 'cases.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length) };
      const answer = await typeOracle(path.join(root, config), { at, properties: false });
      const hook = await compilerHookOracle(path.join(root, config), { operation: 'type', at });
      assert.equal(answer.summary, hook.summary);
      assert.equal(answer.compiler, '7.0.2');
      observed.push({ marker, summary: answer.summary, errorType: answer.errorType, codes: hook.beforeSummary.diagnosticCodes, failures: hook.beforeSummary.demandFailures });
    }
    assert.deepEqual(observed, expected[config]);
    // The alias spelling alone cannot prove the reduced union's members.
    const at = { file: 'cases.ts', ...position(lineStarts(source), source.indexOf('unionExpanded')) };
    const expanded = await typeOracle(path.join(root, config), { at, properties: false });
    const expandedHook = await compilerHookOracle(path.join(root, config), { operation: 'type', at });
    assert.equal(expanded.summary, '"a1" | "b2"');
    assert.equal(expandedHook.summary, expanded.summary);
    assert.deepEqual(expandedHook.beforeSummary.diagnosticCodes, []);
    assert.deepEqual(expandedHook.beforeSummary.demandFailures, []);
    fs.mkdirSync('.cc/temp', { recursive: true });
    fs.writeFileSync(`.cc/temp/native-template-intersections-${config}`, JSON.stringify(observed, null, 2));
  });
}

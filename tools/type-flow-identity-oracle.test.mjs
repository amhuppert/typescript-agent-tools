import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-flow-identity/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
function at(name) {
  const tag = `/*#${name}*/`;
  const index = source.indexOf(tag);
  assert.ok(index >= 0, name);
  return { file: 'cases.ts', ...position(lineStarts(source), index + tag.length) };
}
async function observed(config, marker, symbolIDPrefix = 0) {
  const answer = await compilerHookOracle(path.join(root, config), {
    at: at(marker), operation: 'type', singleThreadBinding: true, symbolIDPrefix,
  });
  assert.equal(answer.compiler, '7.0.2');
  assert.equal(answer.history, 'baseline');
  assert.equal(answer.errorType, false, marker);
  assert.deepEqual(answer.beforeSummary.demandFailures, [], marker);
  assert.ok(answer.beforeSummary.diagnosticCodes.every(code => code === 2318), marker);
  return answer;
}

for (const config of ['tsconfig.json', 'loose.json']) {
  test(`native stable-index eligibility and named-dot byte concatenation: ${config}`, async () => {
    for (const marker of ['stableParameter', 'stableConst', 'stableMutable', 'assignedMutable', 'nestedSuffix']) {
      assert.equal((await observed(config, marker)).summary, 'string | number', marker);
    }
    assert.equal((await observed(config, 'namedDots')).summary,
      '{ warm: string | number; result: string | number; }');
  });
  test(`native binary ID collision can hide a changed dynamic read: ${config}`, async () => {
    const colliding = await observed(config, 'binaryIdentity', 0);
    const separate = await observed(config, 'binaryIdentity', 1);
    assert.deepEqual(colliding.inputHashes, separate.inputHashes);
    assert.deepEqual(colliding.compilerOptions, separate.compilerOptions);
    assert.deepEqual(colliding.beforeSummary.observations.flowKeySymbolID, { id: 2, name: 'key' });
    assert.deepEqual(separate.beforeSummary.observations.flowKeySymbolID, { id: 3, name: 'key' });
    assert.equal(colliding.summary, '{ warm: string | number; result: string | number; }');
    assert.equal(separate.summary, '{ warm: string | number; result: string; }');
  });
}

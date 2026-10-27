import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';
import { scratchPath } from './harness-scratch.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-symbol-id-collision/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');

test('native flow binary key collision changes the final type with prior symbol-ID allocation', async () => {
  const observations = [];
  let baseline;
  for (const symbolIDPrefix of [0, 1, 2]) {
    const tag = '/*#answer*/';
    const answer = await compilerHookOracle(path.join(root, 'tsconfig.json'), {
      at: { file: 'cases.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length) },
      operation: 'type', symbolIDPrefix, singleThreadBinding: true,
    });
    assert.equal(answer.compiler, '7.0.2');
    assert.equal(answer.history, 'baseline');
    assert.ok(answer.beforeSummary.diagnosticCodes.every(code => code === 2318));
    const inputs = { hashes: answer.inputHashes, options: answer.compilerOptions };
    if (baseline === undefined) baseline = inputs;
    else assert.deepEqual(inputs, baseline);
    observations.push({ prefix: symbolIDPrefix, summary: answer.summary,
      key: answer.beforeSummary.observations.flowKeySymbolID,
      errorType: answer.errorType, failures: answer.beforeSummary.demandFailures });
  }
  assert.deepEqual(observations, [
    { prefix: 0, summary: '{ warm: number; result: number; }', key: { id: 2, name: 'key' }, errorType: false, failures: [] },
    { prefix: 1, summary: '{ warm: number; result: string; }', key: { id: 3, name: 'key' }, errorType: false, failures: [] },
    { prefix: 2, summary: '{ warm: number; result: string; }', key: { id: 4, name: 'key' }, errorType: false, failures: [] },
  ]);
});

test('native symbol IDs use eight binary bytes rather than decimal property-name text', async () => {
  const temporary = fs.mkdtempSync(scratchPath('type-symbol-id-decimal-'));
  try {
    const decimal = source.replaceAll('@\\u0002\\u0000\\u0000\\u0000\\u0000\\u0000\\u0000\\u0000', '@2');
    assert.notEqual(decimal, source);
    fs.writeFileSync(path.join(temporary, 'cases.ts'), decimal);
    fs.copyFileSync(path.join(root, 'tsconfig.json'), path.join(temporary, 'tsconfig.json'));
    for (const symbolIDPrefix of [0, 1]) {
      const tag = '/*#answer*/';
      const answer = await compilerHookOracle(path.join(temporary, 'tsconfig.json'), {
        at: { file: 'cases.ts', ...position(lineStarts(decimal), decimal.indexOf(tag) + tag.length) },
        operation: 'type', symbolIDPrefix, singleThreadBinding: true,
      });
      assert.equal(answer.summary, '{ warm: number; result: string; }');
      assert.deepEqual(answer.beforeSummary.observations.flowKeySymbolID, { id: symbolIDPrefix + 2, name: 'key' });
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.beforeSummary.demandFailures, []);
    }
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
});

test('native private-name allocation records concurrent binding and the serial LIFO baseline', async () => {
  const temporary = fs.mkdtempSync(scratchPath('type-symbol-id-binding-'));
  try {
    const count = 12;
    const files = Array.from({ length: count }, (_, i) => `class${i}.ts`);
    for (let i = 0; i < count; i++) {
      fs.writeFileSync(path.join(temporary, files[i]), `export class C${i} { #field = 0; }\n`);
    }
    files.push('answer.ts');
    const answerSource = 'export const /*#answer*/answer = 0;\n';
    fs.writeFileSync(path.join(temporary, 'answer.ts'), answerSource);
    fs.writeFileSync(path.join(temporary, 'tsconfig.json'), JSON.stringify({
      compilerOptions: { strict: true, noLib: true, target: 'esnext', module: 'esnext' }, files,
    }));
    const observations = [];
    for (const singleThreadBinding of [true, true, false, false, false, false]) {
      const tag = '/*#answer*/';
      const answer = await compilerHookOracle(path.join(temporary, 'tsconfig.json'), {
        at: { file: 'answer.ts', ...position(lineStarts(answerSource), answerSource.indexOf(tag) + tag.length) },
        operation: 'type', inspectPrivateNames: true, singleThreadBinding,
      });
      assert.equal(answer.summary, '0');
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.beforeSummary.demandFailures, []);
      const rows = answer.privateNames.map(({ class: name, suffix }) => {
        const match = /^(\d+)@#field$/.exec(suffix);
        assert.ok(match, suffix);
        return { name, id: Number(match[1]) };
      }).sort((left, right) => Number(left.name.slice(1)) - Number(right.name.slice(1)));
      assert.deepEqual(rows.map(row => row.name), Array.from({ length: count }, (_, i) => `C${i}`));
      assert.deepEqual(rows.map(row => row.id).sort((left, right) => left - right),
        Array.from({ length: count }, (_, i) => i + 1));
      if (singleThreadBinding) {
        assert.deepEqual(rows.map(row => row.id), Array.from({ length: count }, (_, i) => count - i));
      }
      observations.push({ singleThreadBinding, rows });
    }
    // Parallel order is intentionally observed, never asserted: a particular
    // scheduling permutation is not a native guarantee.
    fs.writeFileSync(scratchPath('type-symbol-id-bind-observations.json'), JSON.stringify(observations, null, 2));
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
});

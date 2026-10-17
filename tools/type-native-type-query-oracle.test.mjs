import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle, compilerHookOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-type-query/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'queries.ts'), 'utf8');
function at(marker) {
  const comment = `/*${marker}*/`;
  const offset = source.indexOf(comment);
  assert.notEqual(offset, -1, marker);
  assert.equal(source.indexOf(comment, offset + 1), -1, marker);
  const start = offset + comment.length + (source[offset + comment.length] === ' ' ? 1 : 0);
  return { file: 'queries.ts', ...position(lineStarts(source), start) };
}
async function observe(config, marker, kind, summary, errorType = false) {
  const answer = await typeOracle(path.join(directory, config), { at: at(marker), properties: false });
  assert.equal(answer.compiler, '7.0.2');
  assert.equal(answer.history, 'baseline');
  assert.equal(answer.kind, kind, marker);
  assert.equal(answer.summary, summary, marker);
  assert.equal(answer.errorType, errorType, marker);
  assert.deepEqual(answer.prelude, []);
  return answer;
}

test('whole typeof type nodes and their operand value questions preserve literal regularization and mutable types', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [prefix, summary] of [['fixed', '1'], ['count', 'number'], ['import', 'number'],
      ['object', '{ text: string; count: number; }'], ['type-import', '1'], ['type-object', '{ text: string; count: number; }'],
      ['specifier', '1'], ['specifier-object', '{ text: string; count: number; }'],
      ['exported', '1'], ['exported-object', '{ text: string; count: number; }']]) {
      await observe(config, `${prefix}-type`, 'type', summary);
      await observe(config, `${prefix}-value`, 'value', summary);
    }
  }
});

test('typeof initialization suppression retains actual location flow and does not suppress automatic evolution', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const suffix of ['type', 'value']) {
      await observe(config, `pending-${suffix}`, suffix === 'type' ? 'type' : 'value', 'number');
      await observe(config, `automatic-${suffix}`, suffix === 'type' ? 'type' : 'value', config === 'tsconfig.json' ? 'undefined' : 'any');
      await observe(config, `narrowed-${suffix}`, suffix === 'type' ? 'type' : 'value', 'number');
      await observe(config, `choice-${suffix}`, suffix === 'type' ? 'type' : 'value', 'string | number');
    }
    await observe(config, 'ordinary-pending', 'expression', 'number');
  }
});

test('consumed unresolved typeof operands recover error types and instantiation/qualified questions stay distinct', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await observe(config, 'missing-type', 'type', 'any', true);
    await observe(config, 'missing-value', 'value', 'any', true);
    await observe(config, 'instantiated-type', 'type', 'typeof generic<string>');
    await observe(config, 'instantiated-value', 'value', '<T>(value: T) => T');
    await observe(config, 'qualified-type', 'type', 'number');
    await observe(config, 'qualified-value', 'value', 'number');
    await observe(config, 'this-type', 'type', 'this');
    await observe(config, 'this-value', 'value', 'this');
  }
});

test('type-only runtime access and type-argument ancestor stopping preserve the actual native questions', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [marker, summary, code] of [['runtime-type-import', '1', 1361], ['runtime-type-object', '{ text: string; count: number; }', 1361],
      ['runtime-specifier', '1', 1361], ['runtime-specifier-object', '{ text: string; count: number; }', 1361],
      ['runtime-exported', '1', 1362], ['runtime-exported-object', '{ text: string; count: number; }', 1362]]) {
      await observe(config, marker, 'expression', summary);
      const runtime = await compilerHookOracle(path.join(directory, config), { operation: 'type', at: at(marker) });
      assert.equal(runtime.errorType, false);
      assert.ok(runtime.beforeSummary.diagnosticCodes.includes(code), marker);
    }
    for (const marker of ['type-import-value', 'pending-value']) {
      const query = await compilerHookOracle(path.join(directory, config), { operation: 'type', at: at(marker) });
      assert.equal(query.kind, 'value');
      assert.equal(query.errorType, false);
      assert.ok(!query.beforeSummary.diagnosticCodes.includes(1361));
      assert.ok(!query.beforeSummary.diagnosticCodes.includes(2454));
    }
    await observe(config, 'argument-type', 'type', 'MissingArgument', true);
  }
});

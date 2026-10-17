import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/checking-prelude/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'sites.ts'), 'utf8');
const starts = lineStarts(source);
const offset = name => {
  const marker = `/*#${name}*/`;
  assert.ok(source.includes(marker), name);
  return source.indexOf(marker) + marker.length;
};
const at = name => ({ file: 'sites.ts', ...position(starts, offset(name)) });

test('fresh prelude retains actual outer-to-inner roots for expression, value, declared and type questions', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [name, kind, summary, roots] of [
      ['ordinary', 'expression', '1', ['take(']],
      ['nested', 'expression', '2', ['outer(inner(', 'inner(/*#nested*/']],
      ['returnHidden', 'expression', '3', ['hiddenReturn(/*#returnHidden*/']],
      ['missingParameter', 'expression', '4', ['hiddenParameter(']],
      ['rejected', 'expression', '"bad"', ['take(/*#rejected*/']],
      ['outerRejected', 'expression', '5', ['outer(inner(/*#outerRejected*/', 'inner(/*#outerRejected*/']],
      ['thisRejected', 'expression', '7', ['wrongThis(']],
      ['nullable', 'expression', 'null', ['take(/*#nullable*/']],
      ['typeArgument', 'type', 'number', ['generic<']],
      ['genericArgument', 'expression', '8', ['generic<']],
      ['newArgument', 'expression', '9', ['new Made(']],
      ['tagged', 'expression', '"x"', ['tag/*#tagged*/']],
      ['calledReturn', 'expression', 'MissingReturn', ['hiddenReturn(10']],
      ['callbackValue', 'value', '1', ['callback(() => { const /*#callbackValue*/']],
      ['callbackDeclared', 'declared', 'number', ['callback(() => { type ']],
      ['callbackType', 'type', 'number', ['callback(() => { type ']],
    ]) {
      const answer = await typeOracle(path.join(root, config), { at: at(name), properties: false });
      assert.deepEqual([answer.kind, answer.summary], [kind, summary], `${config}:${name}`);
      assert.deepEqual(answer.prelude.map(row => row.location.start),
        roots.map(text => position(starts, source.indexOf(text))), `${config}:${name}: actual roots`);
      assert.deepEqual(answer.prelude.map(row => row.kind),
        roots.map(() => name === 'newArgument' ? 'NewExpression' : name === 'tagged' ? 'TaggedTemplateExpression' : 'CallExpression'));
    }
  }
});

test('same-checker prelude resolves signatures without an extra return demand', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [name, present, absent] of [
      ['returnHidden', [], [2304, 2345, 2684, 2554]],
      ['calledReturn', [2304], [2345, 2684, 2554]],
      ['missingParameter', [2304], [2684, 2554]],
      ['rejected', [2345], [2304, 2684, 2554]],
      ['outerRejected', [2554], [2304, 2345, 2684]],
      ['thisRejected', [2684], [2304, 2345, 2554]],
      ['nullable', config === 'tsconfig.json' ? [2345] : [], [2304, 2684, 2554, ...(config === 'loose.json' ? [2345] : [])]],
    ]) {
      const answer = await compilerHookOracle(path.join(root, config), { operation: 'type', at: at(name) });
      assert.deepEqual(answer.beforeSummary.demandFailures, [], `${config}:${name}`);
      for (const code of present)
        assert.ok(answer.beforeSummary.diagnosticCodes.includes(code), `${config}:${name}: missing ${code}`);
      for (const code of absent)
        assert.ok(!answer.diagnosticCodes.includes(code), `${config}:${name}: unexpected ${code}`);
    }
  }
});

test('a separate declaration summary consumes its actual return annotation without a prelude', async () => {
  const definitions = fs.readFileSync(path.join(root, 'definitions.ts'), 'utf8');
  const own = { file: 'definitions.ts', ...position(lineStarts(definitions), definitions.indexOf('hiddenReturn')) };
  for (const config of ['tsconfig.json', 'loose.json']) {
    const fresh = await typeOracle(path.join(root, config), { at: own, properties: false });
    assert.deepEqual(fresh.prelude, []);
    const answer = await compilerHookOracle(path.join(root, config), { operation: 'type', at: own });
    assert.equal(answer.summary, '(value: number) => MissingReturn');
    assert.equal(answer.errorType, false);
    assert.ok(!answer.beforeSummary.diagnosticCodes.includes(2304));
    assert.ok(answer.diagnosticCodes.includes(2304));
  }
});

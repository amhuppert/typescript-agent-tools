import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';
import { scratchPath } from './harness-scratch.mjs';

const root = fileURLToPath(new URL('../fixtures/types/constant-positions/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'positions.ts'), 'utf8');
const config = path.join(root, 'tsconfig.json');
const offset = marker => source.indexOf(`/*#${marker}*/`) + `/*#${marker}*/`.length;
const request = value => ({ at: { file: 'positions.ts', ...position(lineStarts(source), value) }, properties: false });

test('nested literal tokens keep their expression question and exact own location', async () => {
  for (const [marker, summary] of [['number', '1'], ['twice', '1'], ['big', '42n'], ['boolean', 'false'], ['propertyInitializer', '1'], ['arrayElement', '"array"'], ['conditional', '"branch"'], ['computed', '"member"'], ['returned', '"result"'], ['condition', 'true']]) {
    const answer = await typeOracle(config, request(offset(marker)));
    assert.deepEqual([answer.kind, answer.summary, answer.errorType], ['expression', summary, false], marker);
    assert.deepEqual(answer.location.start, position(lineStarts(source), offset(marker)));
    assert.deepEqual(answer.prelude, []);
  }
  const start = offset('template');
  const end = source.indexOf('`', start + 1);
  for (const value of [start, start + 1, start + 2, end]) {
    const answer = await typeOracle(config, request(value));
    assert.deepEqual([answer.kind, answer.summary, answer.errorType], ['expression', '"A💡"', false]);
    assert.deepEqual(answer.location.start, position(lineStarts(source), start));
    assert.deepEqual(answer.location.end, position(lineStarts(source), end + 1));
  }
});

test('operator and trivia tokens do not acquire nearby expression anchors', async () => {
  for (const value of [offset('operator'), source.indexOf(' /*💡*/') + 1, source.indexOf(' /*💡*/')]) {
    await assert.rejects(typeOracle(config, request(value)), /UnsupportedTarget/);
  }
});

test('name-bearing string tokens consume their own symbol question rather than a cooked string literal', async () => {
  const property = await typeOracle(config, request(offset('propertyKey')));
  assert.equal(property.kind, 'value');
  assert.equal(property.summary, 'number');
  assert.equal(property.errorType, false);
  await assert.rejects(typeOracle(config, request(offset('moduleKey'))), /UnsupportedTarget/);
});

test('a literal argument retains its actual enclosing-call checking prelude', async () => {
  for (const [marker, kind, summary, calls] of [['argument', 'expression', '"ready"', ['CallExpression']], ['nestedCall', 'expression', '"nested"', ['CallExpression', 'CallExpression']], ['newArg', 'expression', '"new"', ['NewExpression']], ['tagToken', 'expression', '"tagged"', ['TaggedTemplateExpression']], ['callbackName', 'value', '1', ['CallExpression']], ['callProperty', 'value', 'number', ['CallExpression']]]) {
    const answer = await typeOracle(config, request(offset(marker)));
    assert.deepEqual([answer.kind, answer.summary, answer.errorType], [kind, summary, false], marker);
    assert.deepEqual(answer.prelude.map(row => row.kind), calls, marker);
  }
});

test('native literal preludes consume healthy globals before selecting the token', async () => {
  const rows = [];
  for (const [marker, kind, summary, calls] of [
    ['argument', 'expression', '"ready"', ['CallExpression']],
    ['nestedCall', 'expression', '"nested"', ['CallExpression', 'CallExpression']],
    ['newArg', 'expression', '"new"', ['NewExpression']],
    ['tagToken', 'expression', '"tagged"', ['TaggedTemplateExpression']],
    ['callbackName', 'value', '1', ['CallExpression']],
    ['callProperty', 'value', 'number', ['CallExpression']],
  ]) {
    const answer = await compilerHookOracle(config, {
      ...request(offset(marker)), history: 'baseline', inspectDiagnosticCache: true,
    });
    assert.deepEqual([answer.kind, answer.summary, answer.errorType], [kind, summary, false], marker);
    const api = await typeOracle(config, request(offset(marker)));
    assert.deepEqual(api.prelude.map(row => row.kind), calls, marker);
    assert.deepEqual(answer.beforeSummary.demandFailures, [], marker);
    assert.deepEqual(answer.beforeSummary.displayAcquisitions.filter(event =>
      event.event === 'exit' && event.errorType), [], marker);
    rows.push({ marker, answer });
  }
  fs.writeFileSync(scratchPath('stage68-constant-positions-native.json'), JSON.stringify(rows, null, 2) + '\n');
});

test('a healthy selected tagged literal can still consume missing-global recovery', async () => {
  const answer = await compilerHookOracle(path.join(root, 'missing-globals.json'), {
    ...request(offset('tagToken')), history: 'baseline', inspectDiagnosticCache: true,
  });
  assert.deepEqual([answer.kind, answer.summary, answer.errorType], ['expression', '"tagged"', false]);
  const api = await typeOracle(path.join(root, 'missing-globals.json'), request(offset('tagToken')));
  assert.deepEqual(api.prelude.map(row => row.kind), ['TaggedTemplateExpression']);
  fs.writeFileSync(scratchPath('stage68-constant-positions-missing-native.json'), JSON.stringify(answer, null, 2) + '\n');
  const errors = answer.beforeSummary.displayAcquisitions.filter(event =>
    event.event === 'exit' && event.errorType);
  const annotation = Buffer.byteLength(source.slice(0, source.indexOf('TemplateStringsArray')), 'utf8');
  assert.ok(errors.some(event => event.operation === 'getTypeFromTypeNode'
    && event.pos <= annotation && event.end === annotation + 'TemplateStringsArray'.length));
  assert.ok(answer.beforeSummary.diagnosticCodes.includes(2304));
});

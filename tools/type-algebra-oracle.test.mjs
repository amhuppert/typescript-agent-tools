// Pinned compiler expectations for T. This does not exercise a native checker.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/type-algebra/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'cases.ts'), 'utf8');
function at(marker) {
  const text = `/*#${marker}*/`;
  const start = source.indexOf(text);
  assert.notEqual(start, -1, marker);
  assert.equal(source.indexOf(text, start + 1), -1, `unique ${marker}`);
  return { file: 'cases.ts', ...position(lineStarts(source), start + text.length) };
}
const observe = (marker, request = {}, config = 'tsconfig.json') =>
  typeOracle(path.join(directory, config), { at: at(marker), properties: false, ...request });
async function valid(marker, summary, kind = 'value', request = {}) {
  const answer = await observe(marker, request);
  assert.equal(answer.summary, summary, marker);
  assert.equal(answer.kind, kind, marker);
  assert.equal(answer.errorType, false, marker);
  assert.deepEqual(answer.diagnostics, [], 'no diagnostic pass precedes or follows this demand');
  return answer;
}
const properties = answer => answer.properties.map(row => [row.name, row.summary, row.errorType]);

test('keyof, indexed, deferred and reduced conditional types retain distinct compiler answers', async () => {
  await valid('keyof', '"first" | "second"', 'declared');
  await valid('indexed', 'number', 'declared');
  await valid('distributed', 'Distributed<T>', 'declared');
  await valid('distribution', '{ text: "s"; } | { other: 1; }');
  await valid('unknownConditional', '{ other: unknown; }');
  await valid('infer', 'number');
});

test('mapped modifier removal and key remapping establish the resulting member types', async () => {
  const modifiers = await valid('modifiers', 'Mutable<{ readonly first?: string | undefined; second?: number | undefined; }>', 'value', { properties: true });
  assert.deepEqual(properties(modifiers), [['first', 'string', false], ['second', 'number', false]]);
  const remap = await valid('remap', 'Remap<{ first: string; second: number; }>', 'value', { properties: true });
  assert.deepEqual(properties(remap), [['getFirst', '() => string', false], ['getSecond', '() => number', false]]);
});

test('template products and intrinsic/global hooks are reduced under effective strict options', async () => {
  for (const [marker, summary] of [
    ['template', '"slot-a-1" | "slot-a-2" | "slot-b-1" | "slot-b-2"'],
    ['uppercase', '"ABC"'], ['lowercase', '"abc"'], ['capitalize', '"Word"'], ['uncapitalize', '"word"'],
    ['awaited', 'string'], ['builtin', 'undefined'],
  ]) await valid(marker, summary);
  const loose = await observe('builtin', {}, 'loose.json');
  assert.equal(loose.compilerOptions.strict, false);
  assert.equal(loose.summary, 'any');
  assert.equal(loose.errorType, false, 'non-strict BuiltinIteratorReturn is genuine any');
});

test('readonly, named optional/rest and variadic tuple types preserve compiler ordering', async () => {
  await valid('tuple', 'readonly [name: string, count?: number | undefined, ...tail: boolean[]]');
  await valid('variadic', '[first: string, number, (boolean | undefined)?]');
  const receiver = await valid('thisType', '{ compare(this: Date, other: Date): boolean; }', 'value', { properties: true });
  assert.deepEqual(properties(receiver), [['compare', '(this: Date, other: Date) => boolean', false]]);
});

test('inference, contextual callbacks and fresh versus const literals produce useful final types', async () => {
  const fresh = await valid('freshObject', '{ text: string; count: number; }', 'value', { properties: true });
  assert.deepEqual(properties(fresh), [['text', 'string', false], ['count', 'number', false]]);
  const constant = await valid('constant', '{ readonly text: "s"; readonly count: 1; }', 'value', { properties: true });
  assert.deepEqual(properties(constant), [['text', '"s"', false], ['count', '1', false]]);
  const factory = await valid('factory', '{ get: () => string; pair: readonly [string, string]; }', 'value', { properties: true });
  assert.deepEqual(properties(factory), [['get', '() => string', false], ['pair', 'readonly [string, string]', false]]);
  await valid('callback', '{ text: string; count: number; }[]');
  await valid('genericClass', 'Map<string, number>');
  await valid('noInfer', '1');
});

test('a non-error root can survive an erroneous NoInfer argument without establishing recovery causality', async () => {
  const cold = await observe('noInferRecovery');
  assert.equal(cold.summary, '1');
  assert.equal(cold.errorType, false);
  assert.deepEqual(cold.diagnostics, []);
  // Diagnostic collection occurs after the selected demand in this fresh request.
  // Its existence is a control, not a product rule for invalidating every root.
  const checked = await observe('noInferRecovery', { diagnostics: true });
  assert.equal(checked.summary, '1');
  assert.equal(checked.errorType, false);
  const diagnostic = checked.diagnostics.find(row => row.code === 2345 && row.location?.start.line === at('noInferRecovery').line);
  assert.ok(diagnostic, 'the erroneous second argument is rejected by the pinned compiler');
});

test('recursive aliases and unbounded generic growth expose distinct next child types', async () => {
  const recursive = await valid('recursive', 'Recursive<string>', 'value', { properties: true });
  assert.deepEqual(properties(recursive), [['value', 'string', false], ['child', 'Recursive<string> | undefined', false]]);
  const growing = await valid('growing', 'Growing<string>', 'value', { properties: true });
  assert.deepEqual(properties(growing), [['value', 'string', false], ['child', 'Growing<string[]>', false]]);
});

test('unique-symbol properties retain declaration origins rather than ephemeral compiler IDs', async () => {
  await valid('symbol', 'unique symbol');
  const keyed = await valid('symbolKey', 'SymbolKey', 'declared', { properties: true });
  assert.equal(keyed.properties.length, 1);
  const property = keyed.properties[0];
  assert.equal(property.summary, 'string');
  assert.equal(property.errorType, false);
  const line = source.split('\n')[at('symbolKey').line - 1];
  const column = line.indexOf('[symbol]') + 1;
  assert.deepEqual(property.origins, [{ kind: 'PropertySignature', location: { file: 'cases.ts',
    start: { line: at('symbolKey').line, column }, end: { line: at('symbolKey').line, column: column + '[symbol]'.length } } }]);
  await valid('enumValue', 'Stage.Next');
});

test('genuine any and unknown stay valid while manufactured unresolved alias any is invalid', async () => {
  await valid('any', 'any');
  await valid('unknown', 'unknown');
  const missing = await observe('missing');
  assert.equal(missing.summary, 'Unresolved');
  assert.equal(missing.errorType, true);
  assert.deepEqual(missing.diagnostics, [], 'root validity does not require a whole-program diagnostic pass');
});

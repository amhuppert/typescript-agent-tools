// Pinned JSON baseline only: this does not exercise a native tool checker.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle, memberOracle, compilerHookOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/json-types/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'cases.ts'), 'utf8');
const json = fs.readFileSync(path.join(directory, 'data.json'), 'utf8');
function at(marker) {
  const token = `/*#${marker}*/`, offset = source.indexOf(token);
  assert.notEqual(offset, -1, marker);
  assert.equal(source.indexOf(token, offset + token.length), -1, marker);
  return { file: 'cases.ts', ...position(lineStarts(source), offset + token.length) };
}
function jsonAt(spelling, nth = 0) {
  let offset = -1;
  for (let index = 0; index <= nth; index++) offset = json.indexOf(spelling, offset + 1);
  assert.notEqual(offset, -1, `${spelling} #${nth}`);
  return { file: 'data.json', ...position(lineStarts(json), offset) };
}
const config = file => path.join(directory, file ?? 'tsconfig.json');
const observe = (marker, request = {}, file) => typeOracle(config(file), { at: at(marker), properties: false, ...request });
const origin = (line, column, endColumn) => ({ kind: 'PropertyAssignment', location: {
  file: 'data.json', start: { line, column }, end: { line, column: endColumn } } });
const topDuplicate = [origin(11, 3, 8), origin(12, 3, 8)];
const nestedDuplicate = [origin(10, 29, 34), origin(10, 48, 53)];
const escapedOrigins = [origin(13, 3, 6), origin(14, 3, 11)];
async function expectType(marker, summary, file) {
  const answer = await observe(marker, {}, file);
  assert.deepEqual([answer.kind, answer.summary, answer.errorType], ['value', summary, false], marker);
  assert.deepEqual(answer.prelude, [], 'these declaration demands have no enclosing call');
  assert.deepEqual(answer.diagnostics, [], 'whole-program diagnostics were not requested');
  return answer;
}

test('imported JSON widens scalar literals and uses the last exact duplicate value', async () => {
  for (const [marker, summary] of [
    ['text', 'string'], ['number', 'number'], ['boolean', 'boolean'], ['nil', 'null'],
    ['duplicate', 'number'], ['nestedDuplicate', '{ last: string; }'], ['escaped', 'boolean'],
    ['emptyKey', 'number'], ['numericCanonical', 'boolean'], ['numericNonCanonical', 'string'],
    ['astral', 'boolean'], ['defaultKey', 'string'], ['namedCopy', 'number'],
    ['rootNull', 'null'], ['rootEmpty', 'never[]'], ['rootNumber', 'number'],
  ]) await expectType(marker, summary);
  const root = await observe('root', { properties: true });
  const properties = new Map(root.properties.map(property => [property.name, property]));
  assert.equal(properties.size, 17);
  assert.equal(properties.get('dup').summary, 'number');
  assert.deepEqual(properties.get('dup').origins, topDuplicate);
  assert.equal(properties.get('a').summary, 'boolean');
  assert.deepEqual(properties.get('a').origins, escapedOrigins);
  assert.equal(properties.get('').summary, 'number');
  assert.equal(properties.get('01').summary, 'string');
  assert.equal(properties.get('1').summary, 'boolean');
});

test('JSON array inference preserves empty, heterogeneous object and recursive array structure', async () => {
  for (const [marker, summary] of [
    ['empty', 'never[]'], ['empty0', 'never'], ['numbers', 'number[]'], ['numbers0', 'number'],
    ['mixed', '(string | number | null)[]'], ['mixed0', 'string | number | null'],
    ['shapes', '({ a: number; b?: undefined; } | { a?: undefined; b: string; })[]'],
    ['shapeA', 'number | undefined'], ['recursive', '(number[] | (string | null)[])[]'],
  ]) await expectType(marker, summary);
});

test('six accepted strict-null, exact-optional and unchecked-index option profiles keep distinct JSON reads', async () => {
  for (const strict of [0, 1]) for (const exact of [0, 1]) for (const unchecked of [0, 1]) {
    if (!strict && exact) continue;
    const file = `strict-${strict}-exact-${exact}-unchecked-${unchecked}.json`;
    const missingSlot = exact ? 'never' : 'undefined';
    for (const [marker, summary] of [
      ['nil', strict ? 'null' : 'any'], ['empty', strict ? 'never[]' : 'any[]'],
      ['rootNull', strict ? 'null' : 'any'], ['rootEmpty', strict ? 'never[]' : 'any[]'],
      ['empty0', !strict ? 'any' : unchecked ? 'undefined' : 'never'],
      ['numbers0', strict && unchecked ? 'number | undefined' : 'number'],
      ['mixed0', !strict ? 'string | number' : unchecked ? 'string | number | null | undefined' : 'string | number | null'],
      ['shapes', `({ a: number; b?: ${missingSlot}; } | { a?: ${missingSlot}; b: string; })[]`],
      ['shapeA', strict ? 'number | undefined' : 'number'],
    ]) {
      const answer = await expectType(marker, summary, file);
      assert.equal(answer.compilerOptions.strictNullChecks, Boolean(strict));
      assert.equal(answer.compilerOptions.exactOptionalPropertyTypes, Boolean(exact));
      assert.equal(answer.compilerOptions.noUncheckedIndexedAccess, Boolean(unchecked));
    }
  }
});

test('exact optional without strict-null checking remains a pinned program-option failure', async () => {
  for (const unchecked of [0, 1])
    await assert.rejects(observe('nil', {}, `strict-0-exact-1-unchecked-${unchecked}.json`), /TS5052/);
});

test('an unchecked JSON element receiver can return a non-error root beside a later read diagnostic', async () => {
  for (const unchecked of [0, 1]) {
    const file = `strict-1-exact-0-unchecked-${unchecked}.json`;
    await expectType('shapeA', 'number | undefined', file);
    const checked = await observe('shapeA', { diagnostics: true }, file);
    assert.deepEqual([checked.summary, checked.errorType], ['number | undefined', false]);
    assert.equal(checked.diagnostics.some(row => row.code === 2532 && row.location?.start.line === at('shapeA').line), Boolean(unchecked));
  }
});

test('missing JSON property recovery stays distinct from unknown and genuine any', async () => {
  for (const [marker, summary, invalid] of [
    ['missing', 'any', true], ['unknown', 'unknown', false],
    ['unknownRead', 'any', true], ['genuineAny', 'any', false],
  ]) {
    const answer = await observe(marker);
    assert.deepEqual([answer.summary, answer.errorType], [summary, invalid], marker);
    assert.deepEqual(answer.diagnostics, []);
  }
  const checked = await observe('missing', { diagnostics: true });
  assert.ok(checked.diagnostics.some(row => row.code === 2339 && row.location?.start.line === at('missing').line));
  assert.ok(checked.diagnostics.some(row => row.code === 18046 && row.location?.start.line === at('unknownRead').line));
});

test('namespace and synthetic default imports retain the native JSON module distinction', async () => {
  const root = await observe('root', { properties: true });
  const synthetic = await observe('syntheticCopy', { properties: true });
  assert.equal(synthetic.summary, root.summary);
  assert.deepEqual(synthetic.properties, root.properties);
  for (const marker of ['importRoot', 'syntheticDefault']) {
    const alias = await observe(marker);
    assert.deepEqual([alias.kind, alias.summary, alias.errorType], ['value', root.summary, false]);
  }
  await expectType('namedDup', 'number');
  const namespace = await observe('namespaceWhole', { properties: true });
  const namespaceDefault = namespace.properties.find(property => property.name === 'default');
  assert.equal(namespaceDefault.summary, root.summary);
  assert.equal(root.properties.find(property => property.name === 'default').summary, 'string');
});

test('Q3 JSON keys preserve every exact origin including duplicates, decoded escape, empty and numeric keys', async () => {
  for (const [anchor, expected] of [
    [jsonAt('"dup"', 0), nestedDuplicate], [jsonAt('"dup"', 1), nestedDuplicate],
    [jsonAt('"dup"', 2), topDuplicate], [jsonAt('"dup"', 3), topDuplicate],
    [jsonAt('"a"', 1), escapedOrigins], [jsonAt('"\\u0061"'), escapedOrigins],
    [jsonAt('""'), [origin(15, 3, 5)]], [jsonAt('"01"'), [origin(16, 3, 7)]],
    [jsonAt('"1"'), [origin(17, 3, 6)]], [jsonAt('"😀"'), [origin(18, 3, 7)]],
    [at('memberDup'), topDuplicate], [at('memberEscaped'), escapedOrigins],
    [at('memberEmpty'), [origin(15, 3, 5)]], [at('memberNumber'), [origin(17, 3, 6)]],
    [at('memberNumericText'), [origin(16, 3, 7)]], [at('memberAstral'), [origin(18, 3, 7)]],
  ]) {
    const answer = await memberOracle(config(), { at: anchor });
    assert.deepEqual(answer.rawOrigins, expected, JSON.stringify(anchor));
    assert.deepEqual(answer.combinedOrigins, expected);
    assert.deepEqual(answer.prelude, []);
  }
});

test('same-checker native controls agree on widened JSON recovery and exact duplicate origins', async () => {
  const controls = [];
  function captured(answer, anchor) {
    for (const file of ['tsconfig.json', 'cases.ts', 'data.json', 'null.json', 'empty-array.json', 'scalar.json']) {
      const absolute = fs.realpathSync(path.join(directory, file));
      const hash = createHash('sha256').update(fs.readFileSync(absolute)).digest('hex');
      assert.equal(answer.inputHashes.find(row => row.file === absolute)?.sha256, hash, `captured ${file}`);
    }
    controls.push({ at: anchor, answer });
  }
  for (const [marker, summary, invalid] of [['duplicate', 'number', false], ['missing', 'any', true], ['genuineAny', 'any', false]]) {
    const answer = await compilerHookOracle(config(), { at: at(marker) });
    assert.deepEqual([answer.kind, answer.summary, answer.errorType], ['value', summary, invalid], marker);
    assert.deepEqual(answer.beforeSummary.demandFailures, []);
    captured(answer, at(marker));
  }
  for (const [anchor, flags] of [[jsonAt('"dup"', 2), 4], [at('memberDup'), 33554436]]) {
    const answer = await compilerHookOracle(config(), { operation: 'member', at: anchor });
    assert.deepEqual(answer.rawOrigins, topDuplicate);
    assert.deepEqual(answer.combinedOrigins, topDuplicate);
    assert.equal(answer.rawSymbolFlags, flags, 'the widened use symbol retains the native transient bit');
    assert.equal(answer.schedule.searchSetCount, 1);
    assert.equal(answer.candidateDemands.length, answer.candidateLedger.length);
    captured(answer, anchor);
  }
  const output = path.resolve('.cc/temp/json-type-go-controls.json');
  fs.mkdirSync(path.dirname(output), { recursive: true });
  fs.writeFileSync(output, JSON.stringify(controls, null, 2));
});

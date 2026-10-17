// Fresh native getReturnTypeFromBody controls; no whole-program diagnostics history.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-return-body/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'consumer.ts'), 'utf8');
function at(name) {
  const matches = [...source.matchAll(new RegExp(`\\b${name}\\b`, 'g'))];
  assert.equal(matches.length, 1, name);
  const offset = matches[0].index;
  return { file: 'consumer.ts', ...position(lineStarts(source), offset) };
}
async function observe(config, name, expected) {
  const answer = await typeOracle(path.join(directory, config), { at: at(name), properties: false });
  assert.equal(answer.compiler, '7.0.2');
  assert.equal(answer.history, 'baseline');
  assert.equal(answer.kind, 'value');
  assert.equal(answer.errorType, false);
  assert.equal(answer.summary, expected, `${config}: ${name}`);
  assert.deepEqual(answer.prelude, []);
  return answer;
}

test('ordered return aggregation widens primitives and objects and excludes nested functions', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [name, expected] of [
      ['importedScalar', '() => number'], ['importedString', '() => string'],
      ['importedBoolean', '(flag: boolean) => boolean'],
      ['importedObject', '() => { text: string; count: number; }'],
      ['importedAllReturns', '(flag: boolean) => 1 | 2'],
      ['importedMixed', '(flag: boolean) => "ready" | 1'],
      ['importedNested', '() => number'], ['importedUnreachable', '() => "unreachable" | 1'],
    ]) await observe(config, name, expected);
  }
});

test('implicit and bare return states distinguish strict undefined and empty void', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    const strict = config === 'tsconfig.json';
    for (const name of ['importedNoReturn', 'importedBareReturn', 'importedThrow', 'importedInfinite'])
      await observe(config, name, '() => void');
    for (const name of ['importedImplicit', 'importedExplicitBare'])
      await observe(config, name, `(flag: boolean) => ${strict ? '1 | undefined' : 'number'}`);
    await observe(config, 'importedUnreachableBare', `() => ${strict ? '1 | undefined' : 'number'}`);
  }
});

test('never contributions and direct bare self calls use their actual function question', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await observe(config, 'importedRecursive', '() => never');
    await observe(config, 'importedRecursiveScalar', '(flag: boolean) => number');
    await observe(config, 'importedNever', '() => never');
    await observe(config, 'importedThrowExpression', '() => never');
    await observe(config, 'importedThrowArrow', '() => never');
  }
});

test('annotation precedence and const/contextual/async/generator operations retain distinct dependencies', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await observe(config, 'importedAnnotated', '() => number');
    await observe(config, 'importedInvalid', '() => any');
    await observe(config, 'importedScalarArrow', '() => number');
    await observe(config, 'importedConstAssertion', '() => "ready"');
    await observe(config, 'importedContextualUndefined', '() => undefined');
    await observe(config, 'importedContextualLiteral', '() => "ready"');
    await observe(config, 'importedAsync', '() => unknown');
    await observe(config, 'importedGenerator', '() => {}');
  }
});

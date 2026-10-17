// Fresh pinned compiler controls for the shared model's admitted algebra.
// These assertions describe compiler facts, not Rust semantic completion.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/model-core/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'cases.ts'), 'utf8');

test('native object truthiness distinguishes callable constructor index and nominal structure from empty anonymous objects', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const name of ['callableResult', 'constructorResult', 'indexedResult', 'emptyInterfaceResult', 'emptyResult']) {
      const answer = await observe(config, name);
      assert.equal(answer.summary, config === 'tsconfig.json' && name !== 'emptyResult' ? 'false' : 'boolean', name);
      assert.equal(answer.errorType, false, name);
    }
  }
});

test('nonprimitive object flags order before structured objects in native unions', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    const answer = await observe(config, 'objectOrdering');
    assert.equal(answer.summary, 'object | { value: string; }');
    assert.equal(answer.errorType, false);
    assert.deepEqual(answer.constituents, [
      { flags: 131072, summary: 'object', errorType: false },
      { flags: 1048576, summary: '{ value: string; }', errorType: false },
    ]);
  }
});

async function observe(configName, name) {
  const semantic = await loadSemanticApi();
  assert.equal(semantic.version, '7.0.2');
  const api = new semantic.API({ tsserverPath: semantic.binary, cwd: directory });
  let snapshot;
  try {
    const config = path.join(directory, configName);
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config);
    assert.ok(project, 'pinned project opens');
    const program = project.program;
    assert.deepEqual([...program.getConfigFileParsingDiagnostics(), ...program.getProgramDiagnostics(),
      ...program.getSyntacticDiagnostics(), ...program.getBindDiagnostics()], []);
    const parsed = program.getSourceFile(path.join(directory, 'cases.ts'));
    const offset = source.includes(`${name}:`) ? source.indexOf(`${name}:`) : source.indexOf(`${name} =`);
    assert.notEqual(offset, -1, name);
    const node = semantic.getTokenAtPosition(parsed, offset);
    const type = project.checker.getTypeOfSymbol(project.checker.getSymbolAtLocation(node));
    const describe = value => ({ flags: value.flags, summary: project.checker.typeToString(value),
      errorType: value.isErrorType() });
    return { ...describe(type), constituents: type.isUnionType() ? type.getTypes().map(describe) : [],
      isArray: project.checker.isArrayType(type),
      properties: project.checker.getPropertiesOfType(type).map(symbol => symbol.name) };
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}

test('intrinsic special types remain successful semantic answers in both null profiles', async () => {
  const expected = { anyValue: [1, 'any'], unknownValue: [2, 'unknown'], neverValue: [262144, 'never'],
    voidValue: [16, 'void'], undefinedValue: [4, 'undefined'], nullValue: [8, 'null'] };
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [name, [flags, summary]] of Object.entries(expected)) {
      const answer = await observe(config, name);
      assert.deepEqual(answer, { flags, summary, errorType: false, constituents: [], isArray: false, properties: [] });
    }
  }
});

test('boolean display preserves the native regular false/true semantic union', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const name of ['booleanValue', 'booleanPair', 'booleanReduction']) {
      const answer = await observe(config, name);
      assert.equal(answer.flags, 134217984);
      assert.equal(answer.summary, 'boolean');
      assert.equal(answer.errorType, false);
      assert.deepEqual(answer.constituents, [
        { flags: 8192, summary: 'false', errorType: false },
        { flags: 8192, summary: 'true', errorType: false },
      ]);
    }
  }
});

test('union construction reduces primitives and orders exact literal values and nullable constituents', async () => {
  const summaries = { stringReduction: 'string', numberReduction: 'number', bigintReduction: 'bigint',
    anyDominates: 'any', unknownDominates: 'unknown', anyUnknown: 'any', neverElided: 'string',
    nestedUnion: 'string | number', stringOrder: '"a" | "b"', numericOrder: '1 | 2' };
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [name, summary] of Object.entries(summaries)) {
      const answer = await observe(config, name);
      assert.equal(answer.summary, summary, `${config} ${name}`);
      assert.equal(answer.errorType, false);
    }
    const mixed = await observe(config, 'mixedOrder');
    assert.equal(mixed.summary, config === 'tsconfig.json'
      ? 'string | number | bigint | boolean | null | undefined'
      : 'string | number | bigint | boolean');
    assert.deepEqual(mixed.constituents.map(value => value.summary), config === 'tsconfig.json'
      ? ['undefined', 'null', 'string', 'number', 'bigint', 'false', 'true']
      : ['string', 'number', 'bigint', 'false', 'true']);
    const nullableVoid = await observe(config, 'voidUndefined');
    assert.equal(nullableVoid.summary, config === 'tsconfig.json' ? 'void | undefined' : 'void');
  }
});

test('missing global Array makes explicit array annotations anonymous objects rather than fabricated arrays', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const name of ['primitiveArray', 'unionArray', 'booleanArray']) {
      assert.deepEqual(await observe(config, name), {
        flags: 1048576, summary: '{}', errorType: false, constituents: [], isArray: false, properties: [],
      });
    }
  }
});

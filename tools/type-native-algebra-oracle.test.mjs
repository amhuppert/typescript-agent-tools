// Fresh pinned native algebra questions precede production type factories.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi, lineStarts, position } from './semantic-oracle.mjs';
import { typeOracle } from './type-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-algebra/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const starts = lineStarts(source);
const sourceHash = createHash('sha256').update(source).digest('hex');
const configs = ['tsconfig.json', 'loose.json'];

function selected(marker) {
  const text = `/*#${marker}*/`;
  const start = source.indexOf(text);
  assert.notEqual(start, -1, marker);
  assert.equal(source.indexOf(text, start + text.length), -1, `unique ${marker}`);
  return { offset: start + text.length, at: { file: 'cases.ts', ...position(starts, start + text.length) } };
}

async function observe(config, marker, expected, errorType = false) {
  const answer = await typeOracle(path.join(root, config), {
    at: selected(marker).at, properties: false, diagnostics: false,
  });
  assert.equal(answer.compiler, '7.0.2');
  assert.equal(answer.history, 'baseline');
  assert.equal(answer.kind, 'value');
  assert.equal(answer.sourceHash, sourceHash);
  assert.equal(answer.summary, expected, `${config}/${marker}`);
  assert.equal(answer.errorType, errorType, `${config}/${marker}`);
  assert.deepEqual(answer.prelude, []);
  assert.deepEqual(answer.diagnostics, []);
}

async function raw(config, markers, demand) {
  const native = await loadSemanticApi();
  const api = new native.API({ tsserverPath: native.binary, cwd: root });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProject: path.join(root, config) });
    const project = snapshot.getProject(path.join(root, config));
    const inputInventories = [
      project.program.getConfigFileParsingDiagnostics(), project.program.getProgramDiagnostics(),
      project.program.getSyntacticDiagnostics(), project.program.getBindDiagnostics(),
    ];
    assert.deepEqual(inputInventories.map(rows => rows.map(row => row.code)), [[], [], [], []]);
    const file = project.program.getSourceFile(path.join(root, 'cases.ts'));
    const checker = project.checker;
    const types = markers.map(marker => {
      const node = native.getTokenAtPosition(file, selected(marker).offset);
      return checker.getTypeOfSymbol(checker.getSymbolAtLocation(node));
    });
    return demand(checker, types);
  } finally {
    try { snapshot?.dispose(); } finally { api.close(); }
  }
}

test('intersection scalar normalization and missing recovery follow native branch order', async () => {
  for (const config of configs) {
    for (const [marker, summary] of [
      ['intersectionSame', 'string'], ['intersectionDisjoint', 'never'],
      ['intersectionUnknown', 'string'], ['intersectionAny', 'any'],
      ['intersectionLiteral', '"ready"'], ['intersectionUnits', 'never'],
      ['intersectionBoolean', 'never'], ['intersectionEmpty', '{}'],
    ]) await observe(config, marker, summary);
    await observe(config, 'intersectionMissing', 'any', true);
  }
});

test('intersection normalization skips unresolved child inventories and preserves the exact empty-literal exception', async () => {
  for (const config of configs) {
    for (const [marker, summary] of [
      ['intersectionAnySkipsIndex', 'any'], ['intersectionNeverSkipsIndex', 'never'],
      ['intersectionAnySkipsReturn', 'any'], ['intersectionNeverSkipsReturn', 'never'],
      ['intersectionLiteralEmptyString', '{} & string'],
      ['intersectionLiteralEmptyNumber', 'number & {}'],
      ['intersectionLiteralEmptyBigint', '{} & bigint'],
      ['intersectionAliasedEmptyString', 'string'],
    ]) await observe(config, marker, summary);
    for (const marker of ['intersectionLiteralEmptyString', 'intersectionLiteralEmptyNumber', 'intersectionLiteralEmptyBigint']) {
      await raw(config, [marker], (_checker, [type]) => {
        assert.equal(type.isIntersectionType(), true);
        assert.equal(type.isErrorType(), false);
      });
    }
  }
});

test('intersection nullable modes and common-null union extraction remain distinct', async () => {
  for (const config of configs) {
    const strict = config === 'tsconfig.json';
    for (const [marker, summary] of [
      ['intersectionNullable', strict ? 'never' : 'null'],
      ['intersectionUndefined', strict ? 'never' : 'undefined'],
      ['intersectionNullUnions', strict ? 'null' : 'never'],
      ['intersectionUndefinedUnions', strict ? 'undefined' : 'never'],
    ]) await observe(config, marker, summary);
  }
});

test('intersection order, exact dedup, alias identity and denormalized union origin are native facts', async () => {
  for (const config of configs) {
    for (const marker of ['intersectionOrder', 'intersectionFlatten']) await observe(config, marker, 'B & A');
    await observe(config, 'intersectionAlias', 'IntersectionAlias');
    await observe(config, 'intersectionOtherAlias', 'OtherIntersectionAlias');
    await raw(config, ['intersectionOrder', 'intersectionFlatten', 'intersectionAlias', 'intersectionOtherAlias', 'intersectionProduct'],
      (checker, [ordered, flattened, alias, otherAlias, product]) => {
        assert.equal(ordered.id, flattened.id);
        assert.equal(ordered.isIntersectionType(), true);
        assert.deepEqual(ordered.getTypes().map(type => checker.typeToString(type)), ['B', 'A']);
        assert.notEqual(alias.id, otherAlias.id);
        assert.equal(alias.getAliasSymbol().name, 'IntersectionAlias');
        assert.equal(otherAlias.getAliasSymbol().name, 'OtherIntersectionAlias');
        assert.equal(product.isUnionType(), true);
        assert.deepEqual(product.getTypes().map(type => checker.typeToString(type)), [
          'A & B', 'A & { delta: bigint; }', '{ gamma: boolean; } & B', '{ gamma: boolean; } & { delta: bigint; }',
        ]);
        assert.equal(checker.typeToString(product), '(A | { gamma: boolean; }) & (B | { delta: bigint; })');
      });
  }
});

test('intersection constraint reduction keeps canonical parameter or actual narrowed intersection', async () => {
  for (const config of configs) {
    await observe(config, 'intersectionConstrainedSame', 'T');
    await observe(config, 'intersectionConstrainedDisjoint', 'never');
    await observe(config, 'intersectionConstrainedNarrow', 'U & "ready"');
    await raw(config, ['intersectionConstrainedSame', 'intersectionConstrainedNarrow'], (checker, [same, narrowed]) => {
      assert.equal(same.isTypeParameter(), true);
      assert.equal(narrowed.isIntersectionType(), true);
      assert.deepEqual(narrowed.getTypes().map(type => checker.typeToString(type)), ['U', '"ready"']);
    });
  }
});

test('tuple target flags, actual arguments, rest collapse and consumed error child are distinct questions', async () => {
  for (const config of configs) {
    const strict = config === 'tsconfig.json';
    for (const [marker, summary] of [
      ['tupleEmpty', '[]'], ['tupleFixed', '[string, number]'],
      ['tupleOptional', strict ? '[string, (number | undefined)?]' : '[string, number?]'],
      ['tupleLabelled', strict ? '[title: string, count?: number | undefined]' : '[title: string, count?: number]'],
      ['tupleReadonly', strict ? 'readonly [title: string, count?: number | undefined]' : 'readonly [title: string, count?: number]'],
      ['tupleFixedRest', '[string, ...number[]]'], ['tupleRestOnly', 'number[]'],
      ['tupleReadonlyRestOnly', 'readonly number[]'], ['tupleVariadic', '[string, ...V]'],
      ['tupleEscapedLabel', '[title: string]'], ['tupleMissing', '[MissingTuple]'],
    ]) await observe(config, marker, summary);
    await raw(config, ['tupleFixed', 'tupleOptional', 'tupleReadonly', 'tupleFixedRest', 'tupleVariadic', 'tupleRestOnly', 'tupleMissing'],
      (checker, [fixed, optional, readonly, rest, variadic, collapsed, missing]) => {
        for (const [type, flags] of [[fixed, [1, 1]], [optional, [1, 2]], [readonly, [1, 2]], [rest, [1, 4]], [variadic, [1, 8]]]) {
          assert.equal(checker.isTupleType(type), true);
          assert.deepEqual(type.getTarget().elementFlags, flags);
        }
        assert.equal(readonly.getTarget().readonly, true);
        assert.equal(fixed.getTarget().readonly, false);
        assert.equal(checker.isTupleType(collapsed), false);
        assert.equal(checker.isArrayType(collapsed), true);
        assert.equal(checker.getTypeArguments(optional)[1].isUnionType(), strict);
        assert.equal(missing.isErrorType(), false);
        assert.equal(checker.getTypeArguments(missing)[0].isErrorType(), true);
      });
  }
});

test('concrete operators reduce while generic index, conditional, mapped and template identities remain symbolic', async () => {
  for (const config of configs) {
    const strict = config === 'tsconfig.json';
    for (const [marker, summary] of [
      ['keyofObject', '"a" | "b"'], ['keyofUnion', '"shared"'], ['keyofIntersection', '"alpha" | "beta"'],
      ['keyofUnknown', 'never'], ['keyofAny', 'string | number | symbol'], ['keyofNever', 'string | number | symbol'],
      ['indexedObject', 'number'], ['indexedOptional', strict ? 'string | undefined' : 'string'],
      ['conditionalTrue', 'number'], ['conditionalFalse', 'boolean'], ['conditionalSkipped', 'number'],
      ['templateResolved', '"head-a" | "head-b"'], ['keyofGeneric', 'keyof T'], ['indexedGeneric', 'T[K]'],
      ['conditionalGeneric', 'T extends string ? number : boolean'],
      ['mappedGeneric', strict ? '{ readonly [P in keyof T]?: T[P] | undefined; }' : '{ readonly [P in keyof T]?: T[P]; }'],
      ['templateGeneric', '`head-${S}`'],
    ]) await observe(config, marker, summary);
    await observe(config, 'indexedMissing', 'any', true);
    await observe(config, 'conditionalDemanded', 'MissingDemanded', true);
    await raw(config, ['keyofGeneric', 'indexedGeneric', 'conditionalGeneric', 'mappedGeneric', 'templateGeneric'],
      (checker, [index, access, conditional, mapped, template]) => {
        assert.equal(index.isIndexType(), true);
        assert.equal(index.getTarget().isTypeParameter(), true);
        assert.equal(access.isIndexedAccessType(), true);
        assert.equal(access.getObjectType().id, index.getTarget().id);
        assert.equal(conditional.isConditionalType(), true);
        assert.equal(conditional.getCheckType().id, index.getTarget().id);
        assert.equal(mapped.objectFlags & 32, 32);
        assert.equal(template.isTemplateLiteralType(), true);
        assert.deepEqual(template.texts, ['head-', '']);
      });
  }
});

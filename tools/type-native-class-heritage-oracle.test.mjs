import assert from 'node:assert/strict';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-class-heritage/', import.meta.url));
async function fresh(profile, name, action) {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  const config = path.join(root, profile);
  const api = new native.API({tsserverPath: native.binary, cwd: root});
  let snapshot;
  try {
    snapshot = api.updateSnapshot({openProject: config});
    const project = snapshot.getProject(config);
    assert.deepEqual([...project.program.getConfigFileParsingDiagnostics(),
      ...project.program.getProgramDiagnostics(), ...project.program.getSyntacticDiagnostics(),
      ...project.program.getBindDiagnostics()], []);
    const source = project.program.getSourceFile(path.join(root, 'cases.ts'));
    const declaration = source.statements.flatMap(statement => statement.declarationList?.declarations ?? [])
      .find(declaration => declaration.name.text === name);
    assert.ok(declaration, name);
    const checker = project.checker;
    const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(declaration.name));
    assert.equal(type.isErrorType(), false, name);
    await action({native, checker, type, project, source});
  } finally {
    try { snapshot?.dispose(); } finally { api.close(); }
  }
}

// Product graph labels append this actual symbol's first native declaration;
// native typeToString (and the top-level product summary) keep bare spelling.
function assertClassOrigin(context, type, name) {
  const declaration = type.getSymbol()?.declarations?.[0]?.resolve(context.project);
  assert.ok(declaration, `${name} has its actual nominal declaration`);
  assert.equal(declaration.name?.text, name);
  const expected = context.source.text.indexOf(`class ${name}`);
  assert.notEqual(expected, -1, name);
  assert.equal(declaration.name.getStart(context.source), expected + 'class '.length, name);
}

test('class instance inheritance keeps generic substitutions, merged declarations and own order', async () => {
  for (const profile of ['tsconfig.json', 'loose.json']) {
    for (const [name, summary, properties] of [
      ['derivedInstance', 'Derived', [['own', 'boolean'], ['base', 'string'], ['guarded', 'boolean'], ['hidden', 'number']]],
      ['explicitInstance', 'Explicit', [['own', 'string'], ['base', 'number'], ['guarded', 'boolean'], ['hidden', 'number']]],
      ['mergedInstance', 'Merged', [['own', 'boolean'], ['interfaceOwn', 'number'], ['base', 'number'], ['guarded', 'boolean'], ['hidden', 'number'], ['interfaceBase', 'string']]],
      ['genericInstance', 'GenericDerived<string>', [['own', 'string'], ['base', 'string'], ['guarded', 'boolean'], ['hidden', 'number']]],
      ['likeInstance', 'LikeDerived', [['own', 'boolean'], ['inherited', 'number']]],
    ]) await fresh(profile, name, ({checker, type}) => {
      assert.equal(checker.typeToString(type), summary, name);
      assert.deepEqual(checker.getPropertiesOfType(type).map(property =>
        [property.name, checker.typeToString(checker.getTypeOfSymbol(property))]), properties, name);
    });
  }
});

test('class static exports inherit separately and keep the native prototype factory', async () => {
  for (const profile of ['tsconfig.json', 'loose.json']) {
    for (const [name, className, properties] of [
      ['derivedStatic', 'Derived', [['replaced', '"derived"'], ['inheritedStatic', 'string'], ['prototype', 'Derived']]],
      ['explicitStatic', 'Explicit', [['inheritedStatic', 'string'], ['replaced', 'string'], ['prototype', 'Explicit']]],
      ['likeStatic', 'LikeDerived', [['shared', 'string'], ['prototype', 'LikeDerived']]],
      ['genericStatic', 'GenericDerived', [['inheritedStatic', 'string'], ['replaced', 'string'], ['prototype', 'GenericDerived<any>']]],
    ]) await fresh(profile, name, context => {
      const {native, checker, type} = context;
      assert.equal(checker.typeToString(type), `typeof ${className}`, name);
      const actual = checker.getPropertiesOfType(type);
      assert.deepEqual(actual.map(property => [property.name, checker.typeToString(checker.getTypeOfSymbol(property))]), properties, name);
      const prototype = actual.find(property => property.name === 'prototype');
      assert.ok(prototype.flags & native.SymbolFlags.Prototype, name);
      assert.deepEqual(prototype.declarations, [], name);
      assertClassOrigin(context, checker.getTypeOfSymbol(prototype), className);
    });
  }
});

test('explicit and inherited default constructors retain parameters, abstract flags and class returns', async () => {
  for (const profile of ['tsconfig.json', 'loose.json']) {
    for (const [name, returned, parameters, abstract_, locals] of [
      ['derivedStatic', 'Derived', ['string'], false, []],
      ['explicitStatic', 'Explicit', ['boolean', profile === 'tsconfig.json' ? 'number | undefined' : 'number'], false, []],
      ['abstractStatic', 'Abstract', ['boolean'], true, []],
      ['concreteStatic', 'Concrete', ['boolean'], false, []],
      ['likeStatic', 'LikeDerived', ['number'], false, []],
      ['genericStatic', 'GenericDerived<U>', ['U'], false, ['U']],
    ]) await fresh(profile, name, context => {
      const {native, checker, type} = context;
      const constructors = checker.getSignaturesOfType(type, native.SignatureKind.Construct);
      assert.equal(constructors.length, 1, name);
      const signature = constructors[0];
      assert.equal(signature.isAbstract, abstract_, name);
      assert.equal(signature.isConstruct, true, name);
      assert.deepEqual(signature.getTypeParameters().map(type => checker.typeToString(type)), locals, name);
      assert.deepEqual(signature.getParameters().map(parameter => checker.typeToString(checker.getTypeOfSymbol(parameter))), parameters, name);
      const returnType = checker.getReturnTypeOfSignature(signature);
      assert.equal(checker.typeToString(returnType), returned, name);
      assertClassOrigin(context, returnType, returned.split('<')[0]);
    });
  }
});

test('absent null and any bases retain their distinct inventories and default constructors', async () => {
  for (const profile of ['tsconfig.json', 'loose.json']) {
    for (const [name, className, own] of [['plain', 'Plain', 'number'], ['null', 'NullBase', 'string'], ['any', 'AnyDerived', 'boolean']]) {
      await fresh(profile, `${name}Instance`, ({native, checker, type}) => {
        assert.deepEqual(checker.getPropertiesOfType(type).map(property => [property.name, checker.typeToString(checker.getTypeOfSymbol(property))]), [['own', own]], name);
        const indexes = checker.getIndexInfosOfType(type);
        assert.deepEqual(indexes.map(index => [checker.typeToString(index.keyType), checker.typeToString(index.valueType)]), name === 'any' ? [['string', 'any']] : [], name);
      });
      await fresh(profile, `${name}Static`, context => {
        const {native, checker, type} = context;
        assert.equal(checker.typeToString(type), `typeof ${className}`);
        const constructors = checker.getSignaturesOfType(type, native.SignatureKind.Construct);
        assert.equal(constructors.length, 1, name);
        assert.deepEqual(constructors[0].getParameters(), [], name);
        const returnType = checker.getReturnTypeOfSignature(constructors[0]);
        assert.equal(checker.typeToString(returnType), className, name);
        assertClassOrigin(context, returnType, className);
        const indexes = checker.getIndexInfosOfType(type);
        assert.deepEqual(indexes.map(index => [checker.typeToString(index.keyType), checker.typeToString(index.valueType)]), name === 'any' ? [['string', 'any']] : [], name);
      });
    }
  }
});

test('abstract default constructor flags affect final overload selection', async () => {
  for (const profile of ['tsconfig.json', 'loose.json']) {
    for (const result of ['abstract', 'concrete']) await fresh(profile, `${result}Result`, ({checker, type}) => {
      assert.equal(checker.typeToString(type), `"${result}"`, result);
    });
  }
});

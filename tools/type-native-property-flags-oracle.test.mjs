import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, loadSemanticApi, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-property-flags/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
function request(marker) {
  const label = `/*#${marker}*/`;
  return {at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(label) + label.length)}, properties: false};
}

test('native prototype reads and writes retain ordinary writable property types', async () => {
  for (const config of ['tsconfig.json', 'loose.json', 'exact.json']) {
    for (const [marker, summary] of [['prototypeRead', 'Plain'], ['prototypeWrite', 'Plain'], ['genericPrototypeWrite', 'Generic<any>'],
      ['annotatedPrototypeRead', 'AnnotatedPrototype'], ['getterStringRead', 'string'],
      ['accessorStringRead', 'string'], ['accessorStringWrite', 'string']]) {
      const answer = await typeOracle(path.join(root, config), request(marker));
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.kind, 'expression', marker);
      assert.equal(answer.errorType, false, marker);
      assert.equal(answer.summary, summary, marker);
    }
  }
});

test('native readonly follows independent property flags and actual declaration order', async () => {
  for (const config of ['tsconfig.json', 'loose.json', 'exact.json']) {
    for (const [marker, summary, errorType] of [
      ['propertyFirstWrite', 'any', true], ['setterFirstWrite', 'boolean', false],
      ['withGetterWrite', 'string', false], ['getterOnlyWrite', 'any', true],
      ['mutableWrite', 'boolean', false], ['genericReadonlyWrite', 'any', true],
      ['readonlyPrototypeWrite', 'any', true], ['getterPrototypeWrite', 'any', true],
      ['accessorPrototypeWrite', 'AccessorPrototype', false],
    ]) {
      const answer = await typeOracle(path.join(root, config), request(marker));
      assert.equal(answer.errorType, errorType, marker);
      assert.equal(answer.summary, summary, marker);
    }
  }
});

test('native class declaration names select declared instance types', async () => {
  for (const [marker, summary] of [
    ['plainClass', 'Plain'], ['genericClass', 'Generic<T>'], ['derivedClass', 'Derived'],
    ['expressionClass', 'Inside'], ['genericExpressionClass', 'InsideGeneric<T>'], ['defaultClass', 'NamedDefault'],
  ]) {
    const answer = await typeOracle(path.join(root, 'tsconfig.json'), request(marker));
    assert.equal(answer.kind, 'declared', marker);
    assert.equal(answer.errorType, false, marker);
    assert.equal(answer.summary, summary, marker);
  }
});

test('native static prototype inventories retain declared flags and source nodes', async () => {
  const native = await loadSemanticApi();
  for (const profile of ['tsconfig.json', 'loose.json', 'exact.json']) {
    for (const [name, className, valueType, accessors, declarationCount] of [
      ['plainStatic', 'Plain', 'Plain', 0, 0],
      ['readonlyStatic', 'ReadonlyPrototype', 'ReadonlyPrototype', 0, 1],
      ['getterStatic', 'GetterPrototype', 'GetterPrototype', native.SymbolFlags.GetAccessor, 1],
      ['accessorStatic', 'AccessorPrototype', 'AccessorPrototype', native.SymbolFlags.GetAccessor | native.SymbolFlags.SetAccessor, 2],
      ['annotatedStatic', 'AnnotatedPrototype', 'AnnotatedPrototype', 0, 1],
      ['getterStringStatic', 'GetterStringPrototype', 'string', native.SymbolFlags.GetAccessor, 1],
      ['accessorStringStatic', 'AccessorStringPrototype', 'string', native.SymbolFlags.GetAccessor | native.SymbolFlags.SetAccessor, 2],
    ]) {
      const config = path.join(root, profile);
      const api = new native.API({tsserverPath: native.binary, cwd: root});
      let snapshot;
      try {
        snapshot = api.updateSnapshot({openProject: config});
        const project = snapshot.getProject(config);
        const file = project.program.getSourceFile(path.join(root, 'cases.ts'));
        const declaration = file.statements.flatMap(statement => statement.declarationList?.declarations ?? [])
          .find(declaration => declaration.name.text === name);
        assert.ok(declaration, name);
        const checker = project.checker;
        const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(declaration.name));
        assert.equal(checker.typeToString(type), `typeof ${className}`, name);
        const prototype = checker.getPropertiesOfType(type).find(property => property.name === 'prototype');
        assert.ok(prototype.flags & native.SymbolFlags.Prototype, name);
        assert.ok(prototype.flags & native.SymbolFlags.Property, name);
        assert.equal(prototype.flags & native.SymbolFlags.Accessor, accessors, name);
        assert.equal(prototype.declarations.length, declarationCount, name);
        assert.equal(checker.typeToString(checker.getTypeOfSymbol(prototype)), valueType, name);
      } finally {
        try { snapshot?.dispose(); } finally { api.close(); }
      }
    }
  }
});

test('native class prototype installation precedes later namespace declarations', async () => {
  const native = await loadSemanticApi();
  for (const [name, diagnostics, addedFlag] of [['forward', [], native.SymbolFlags.Interface], ['reverse', [2300], 0],
    ['forward-value', [2300], 0], ['reverse-value', [2300], 0],
    ['forward-alias', [], native.SymbolFlags.Alias], ['reverse-alias', [2300], 0],
    ['forward-type-alias', [], native.SymbolFlags.TypeAlias], ['reverse-type-alias', [2300], 0]]) {
    const config = path.join(root, `${name}.json`);
    const api = new native.API({tsserverPath: native.binary, cwd: root});
    let snapshot;
    try {
      snapshot = api.updateSnapshot({openProject: config});
      const project = snapshot.getProject(config);
      const binds = project.program.getBindDiagnostics();
      assert.deepEqual([...new Set(binds.map(item => item.code))], diagnostics, name);
      const file = project.program.getSourceFile(path.join(root, `${name}.ts`));
      const declaration = file.statements.find(node => node.kind === native.SyntaxKind.ClassDeclaration);
      const symbol = project.checker.getSymbolAtLocation(declaration.name);
      const prototype = symbol.getExports().get('prototype');
      assert.ok(prototype.flags & native.SymbolFlags.Prototype, name);
      assert.equal(prototype.flags & (native.SymbolFlags.Interface | native.SymbolFlags.Alias | native.SymbolFlags.TypeAlias), addedFlag, name);
      assert.equal(prototype.declarations.length, addedFlag ? 1 : 0, name);
    } finally {
      try { snapshot?.dispose(); } finally { api.close(); }
    }
  }
});

test('native prototype modifier and readonly returns are observed during an actual write', async () => {
  const native = await loadSemanticApi();
  const answer = await compilerHookOracle(path.join(root, 'tsconfig.json'), request('prototypeWrite'));
  assert.equal(answer.errorType, false);
  assert.equal(answer.summary, 'Plain');
  assert.deepEqual(answer.beforeSummary.observations.prototypeReadonly, {readonly: false});
  // checkPropertyAccessibility asks the write modifier question before
  // isAssignmentToReadonlyEntity asks isReadonlySymbol on the same property.
  assert.deepEqual(answer.beforeSummary.observations.prototypeModifiers,
    {flags: native.ModifierFlags.Public | native.ModifierFlags.Static, writing: true});
});

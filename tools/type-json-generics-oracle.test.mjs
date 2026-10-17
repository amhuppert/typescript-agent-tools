import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/json-generic-targets/', import.meta.url));

// Native baseline only. Each invocation begins with a fresh checker and the
// four input inventories; it never requests whole-file semantic diagnostics.
async function withArray(name, action, profile = [1, 0, 0]) {
  const native = await loadSemanticApi();
  const directory = path.join(root, name);
  const [strict, exact, unchecked] = profile;
  const config = path.join(directory, `strict-${strict}-exact-${exact}-unchecked-${unchecked}.json`);
  const api = new native.API({ tsserverPath: native.binary, cwd: directory });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config);
    assert.ok(project);
    const input = [...project.program.getConfigFileParsingDiagnostics(), ...project.program.getProgramDiagnostics(), ...project.program.getSyntacticDiagnostics(), ...project.program.getBindDiagnostics()];
    if (!strict && exact) {
      assert.deepEqual(input.map(diagnostic => diagnostic.code), [5052]);
      return; // This profile is not admitted to a semantic question.
    }
    assert.deepEqual(input, []);
    const c = project.checker;
    const source = project.program.getSourceFile(path.join(directory, 'consumer.ts'));
    const marker = '/*#numbers*/';
    const node = native.getTokenAtPosition(source, source.text.indexOf(marker) + marker.length);
    const alias = c.getSymbolAtLocation(node);
    assert.ok(alias);
    const type = c.getTypeOfSymbol(c.getAliasedSymbol(alias));
    assert.equal(c.typeToString(type), 'number[]');
    assert.equal(type.isErrorType(), false);
    assert.equal(type.flags, native.TypeFlags.Object);
    assert.equal(c.isArrayType(type), true);
    const target = type.getTarget();
    assert.equal(target.getSymbol().name, 'Array');
    const parameters = target.getTypeParameters();
    assert.equal(parameters.length, 1);
    assert.equal(c.typeToString(parameters[0]), 'T');
    assert.equal(parameters[0].isTypeParameter(), true);
    assert.equal(parameters[0].isErrorType(), false);
    const arguments_ = c.getTypeArguments(type);
    assert.equal(arguments_.length, 1);
    assert.equal(c.typeToString(arguments_[0]), 'number');
    assert.equal(arguments_[0].isErrorType(), false);
    return action({ native, project, c, type, target, parameter: parameters[0], element: arguments_[0], directory });
  } finally {
    try { snapshot?.dispose(); } finally { api.close(); }
  }
}

test('native JSON Array factory preserves real global target and element identities in every admitted profile', async () => {
  const cases = JSON.parse(fs.readFileSync(path.join(root, 'cases.json'), 'utf8'));
  for (const name of cases) for (const strict of [0, 1]) for (const exact of [0, 1]) for (const unchecked of [0, 1])
    await withArray(name, () => {}, [strict, exact, unchecked]);
});

test('generic Array identity and property inventory defer unrelated constraint, default and member annotations', async () => {
  for (const name of ['unused-constraint', 'unused-default', 'unused-member']) {
    await withArray(name, ({ c, type, element }) => {
      assert.deepEqual(c.getPropertiesOfType(type).map(symbol => symbol.name), name === 'unused-member' ? ['value', 'broken'] : ['value']);
      const value = c.getPropertyOfType(type, 'value');
      assert.ok(value);
      assert.equal(c.getTypeOfSymbol(value).id, element.id);
    });
  }
  await withArray('unused-constraint', ({ c, parameter }) => {
    const constraint = c.getConstraintOfTypeParameter(parameter);
    assert.ok(constraint);
    assert.equal(constraint.isErrorType(), true, 'explicit constraint demand consumes unresolved recovery');
    assert.equal(constraint.flags, 1);
  });
  await withArray('unused-default', ({ native, project, c, directory }) => {
    const globals = project.program.getSourceFile(path.join(directory, 'globals.ts'));
    const declaration = globals.statements.find(statement => statement.kind === native.SyntaxKind.InterfaceDeclaration);
    assert.ok(declaration.typeParameters[0].defaultType);
    assert.equal(c.getTypeAtLocation(declaration.typeParameters[0].defaultType).isErrorType(), true,
      'this is an explicit default-node type question, not GetDefaultFromTypeParameter');
  });
  await withArray('unused-member', ({ c, type }) => {
    const broken = c.getPropertyOfType(type, 'broken');
    assert.ok(broken);
    assert.equal(c.getTypeOfSymbol(broken).isErrorType(), true, 'explicit member read consumes unresolved recovery');
  });
});

test('generic interface and class Array references substitute element and synthetic this through real member questions', async () => {
  for (const name of ['basic', 'valid-class']) await withArray(name, ({ native, c, type, element }) => {
    assert.deepEqual(c.getPropertiesOfType(type).map(symbol => symbol.name), ['value', 'length', 'first', 'self']);
    const property = name => {
      const symbol = c.getPropertyOfType(type, name);
      assert.ok(symbol);
      return c.getTypeOfSymbol(symbol);
    };
    assert.equal(property('value').id, element.id);
    assert.equal(property('length').id, element.id);
    assert.equal(property('self').id, type.id, 'native padded this argument is the exact instantiated root');
    const indexes = c.getIndexInfosOfType(type);
    assert.equal(indexes.length, 1);
    assert.equal(indexes[0].keyType.id, element.id);
    assert.equal(indexes[0].valueType.id, element.id);
    assert.equal(indexes[0].isReadonly, false);
    const signatures = c.getSignaturesOfType(property('first'), native.SignatureKind.Call);
    assert.equal(signatures.length, 1);
    assert.deepEqual(signatures[0].getParameters(), []);
    assert.equal(c.getReturnTypeOfSignature(signatures[0]).id, element.id);
    assert.deepEqual(c.getSignaturesOfType(type, native.SignatureKind.Call), []);
    assert.deepEqual(c.getSignaturesOfType(type, native.SignatureKind.Construct), []);
  });
});

test('merged Array parameter declarations share identity and preserve later constraint or default questions', async () => {
  for (const name of ['merged-later-constraint', 'merged-later-default']) await withArray(name, ({ native, project, c, type, parameter, element, directory }) => {
    const globals = project.program.getSourceFile(path.join(directory, 'globals.ts'));
    const declarations = globals.statements.filter(statement => statement.kind === native.SyntaxKind.InterfaceDeclaration);
    assert.equal(declarations.length, 2);
    for (const declaration of declarations) {
      const symbol = c.getSymbolAtLocation(declaration.typeParameters[0].name);
      assert.equal(c.getDeclaredTypeOfSymbol(symbol).id, parameter.id);
    }
    assert.deepEqual(c.getPropertiesOfType(type).map(symbol => symbol.name), ['value', 'later']);
    assert.equal(c.getTypeOfSymbol(c.getPropertyOfType(type, 'later')).id, element.id);
    if (name === 'merged-later-constraint') {
      assert.equal(declarations[0].typeParameters[0].constraint, undefined);
      const constraint = c.getConstraintOfTypeParameter(parameter);
      assert.ok(constraint);
      assert.equal(c.typeToString(constraint), 'string');
      assert.equal(constraint.isErrorType(), false);
    } else {
      assert.equal(declarations[0].typeParameters[0].defaultType, undefined);
      const defaultNode = declarations[1].typeParameters[0].defaultType;
      assert.ok(defaultNode);
      assert.equal(c.typeToString(c.getTypeAtLocation(defaultNode)), '"ready"');
    }
  });
});

import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-signature-mapper/', import.meta.url));

async function fresh(config, action) {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  const api = new native.API({ tsserverPath: native.binary, cwd: root });
  let snapshot;
  try {
    const configPath = path.join(root, config);
    snapshot = api.updateSnapshot({ openProject: configPath });
    const project = snapshot.getProject(configPath);
    assert.ok(project);
    for (const diagnostics of [project.program.getConfigFileParsingDiagnostics(),
      project.program.getProgramDiagnostics(), project.program.getSyntacticDiagnostics(),
      project.program.getBindDiagnostics()]) assert.deepEqual(diagnostics, []);
    const source = project.program.getSourceFile(path.join(root, 'cases.ts'));
    const checker = project.checker;
    const interfaceType = name => {
      const declaration = source.statements.find(node => node.name?.text === name);
      assert.ok(declaration, name);
      return checker.getDeclaredTypeOfSymbol(checker.getSymbolAtLocation(declaration.name));
    };
    const variableType = name => {
      const declaration = source.statements.flatMap(node => node.declarationList?.declarations ?? [])
        .find(node => node.name.text === name);
      assert.ok(declaration, name);
      return checker.getTypeOfSymbol(checker.getSymbolAtLocation(declaration.name));
    };
    const describe = type => ({ summary: checker.typeToString(type), flags: type.flags,
      errorType: type.isErrorType() });
    await action({ native, checker, source, project, interfaceType, variableType, describe });
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}

test('native generic references substitute every ordinary signature question through the target mapper', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await fresh(config, ({ native, checker, variableType, describe }) => {
      const type = variableType('simple');
      assert.deepEqual(describe(type), { summary: 'Simple<string>', flags: native.TypeFlags.Object, errorType: false });
      const calls = checker.getSignaturesOfType(type, native.SignatureKind.Call);
      const constructs = checker.getSignaturesOfType(type, native.SignatureKind.Construct);
      assert.equal(calls.length, 1); assert.equal(constructs.length, 1);
      assert.deepEqual(calls[0].getTypeParameters(), []);
      for (const signature of [calls[0], constructs[0]]) {
        assert.equal(signature.getParameters().length, 1);
        assert.deepEqual(describe(checker.getTypeOfSymbol(signature.getParameters()[0])),
          { summary: 'string', flags: native.TypeFlags.String, errorType: false });
        assert.deepEqual(describe(checker.getReturnTypeOfSignature(signature)),
          { summary: 'string', flags: native.TypeFlags.String, errorType: false });
      }
      assert.deepEqual(describe(checker.getTypeOfSymbol(calls[0].getThisParameter())),
        { summary: 'string', flags: native.TypeFlags.String, errorType: false });
      assert.equal(calls[0].isConstruct, false); assert.equal(constructs[0].isConstruct, true);
    });
  }
});

test('native signature instantiation clones local canonical parameter identities and maps outer constraints', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await fresh(config, ({ native, checker, interfaceType, variableType, describe }) => {
      const declared = checker.getSignaturesOfType(interfaceType('Generic'), native.SignatureKind.Call)[0];
      const original = declared.getTypeParameters()[0];
      const stringType = variableType('genericString');
      const stringSignature = checker.getSignaturesOfType(stringType, native.SignatureKind.Call)[0];
      const stringParameter = stringSignature.getTypeParameters()[0];
      const numberSignature = checker.getSignaturesOfType(variableType('genericNumber'), native.SignatureKind.Call)[0];
      const numberParameter = numberSignature.getTypeParameters()[0];
      assert.notEqual(stringParameter.id, original.id);
      assert.notEqual(numberParameter.id, original.id);
      assert.notEqual(stringParameter.id, numberParameter.id);
      assert.equal(stringParameter.getSymbol().id, original.getSymbol().id);
      assert.equal(numberParameter.getSymbol().id, original.getSymbol().id);
      assert.equal(checker.getSignaturesOfType(stringType, native.SignatureKind.Call)[0].getTypeParameters()[0].id, stringParameter.id);
      for (const [signature, parameter, expected, flags] of [
        [stringSignature, stringParameter, 'string', native.TypeFlags.String],
        [numberSignature, numberParameter, 'number', native.TypeFlags.Number],
      ]) {
        assert.deepEqual(describe(parameter), { summary: 'U', flags: native.TypeFlags.TypeParameter, errorType: false });
        assert.equal(checker.getTypeOfSymbol(signature.getParameters()[0]).id, parameter.id);
        assert.equal(checker.getReturnTypeOfSignature(signature).id, parameter.id);
        assert.deepEqual(describe(checker.getConstraintOfTypeParameter(parameter)),
          { summary: expected, flags, errorType: false });
      }
    });
  }
});

test('native local clone composition preserves parameter chains and spelling shadowing', async () => {
  await fresh('tsconfig.json', ({ native, checker, variableType, describe }) => {
    const signature = checker.getSignaturesOfType(variableType('nested'), native.SignatureKind.Call)[0];
    const [first, second] = signature.getTypeParameters();
    assert.notEqual(first.id, second.id);
    assert.equal(checker.getTypeOfSymbol(signature.getParameters()[0]).id, first.id);
    assert.equal(checker.getTypeOfSymbol(signature.getParameters()[1]).id, second.id);
    assert.equal(checker.getReturnTypeOfSignature(signature).id, second.id);
    assert.deepEqual(describe(checker.getConstraintOfTypeParameter(first)),
      { summary: 'number', flags: native.TypeFlags.Number, errorType: false });
    assert.equal(checker.getConstraintOfTypeParameter(second).id, first.id);
    const shadow = checker.getSignaturesOfType(variableType('shadow'), native.SignatureKind.Call)[0];
    const local = shadow.getTypeParameters()[0];
    assert.equal(checker.getTypeOfSymbol(shadow.getParameters()[0]).id, local.id);
    assert.deepEqual(describe(checker.getConstraintOfTypeParameter(local)),
      { summary: 'string', flags: native.TypeFlags.String, errorType: false });
  });
});

test('native instantiated signature construction leaves missing constraint and default annotations lazy', async () => {
  await fresh('tsconfig.json', ({ native, checker, variableType, describe }) => {
    const type = variableType('lazy');
    assert.deepEqual(describe(type), { summary: 'Lazy<string>', flags: native.TypeFlags.Object, errorType: false });
    const signature = checker.getSignaturesOfType(type, native.SignatureKind.Call)[0];
    const local = signature.getTypeParameters()[0];
    assert.deepEqual(describe(local), { summary: 'U', flags: native.TypeFlags.TypeParameter, errorType: false });
    assert.equal(checker.getTypeOfSymbol(signature.getParameters()[0]).id, local.id);
    assert.deepEqual(describe(checker.getReturnTypeOfSignature(signature)),
      { summary: 'string', flags: native.TypeFlags.String, errorType: false });
    assert.equal(checker.getConstraintOfTypeParameter(local).isErrorType(), true);
  });
});

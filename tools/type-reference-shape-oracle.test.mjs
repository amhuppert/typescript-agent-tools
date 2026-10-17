import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/reference-shape/', import.meta.url));
async function observe(name, configName) {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  const api = new native.API({ tsserverPath: native.binary, cwd: root });
  let snapshot;
  try {
    const config = path.join(root, configName);
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config);
    assert.ok(project);
    for (const diagnostics of [project.program.getConfigFileParsingDiagnostics(), project.program.getProgramDiagnostics(),
      project.program.getSyntacticDiagnostics(), project.program.getBindDiagnostics()]) assert.deepEqual(diagnostics, []);
    const source = project.program.getSourceFile(path.join(root, 'cases.ts'));
    const statement = source.statements.find(node => node.declarationList?.declarations.some(declaration => declaration.name.text === name));
    assert.ok(statement, name);
    const declaration = statement.declarationList.declarations.find(declaration => declaration.name.text === name);
    const checker = project.checker;
    const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(declaration.name));
    const describe = type => ({ summary: checker.typeToString(type), errorType: type.isErrorType() });
    const before = describe(type);
    // getSignaturesOfType resolves the full native inventory. It does not
    // consume ordinary property values but can consume index annotations.
    const call = checker.getSignaturesOfType(type, native.SignatureKind.Call);
    const construct = checker.getSignaturesOfType(type, native.SignatureKind.Construct);
    const indexes = checker.getIndexInfosOfType(type).map(index => describe(index.valueType));
    const properties = checker.getPropertiesOfType(type);
    return { before, after: describe(type), calls: call.length, constructs: construct.length, indexes,
      callParameter: call[0] && describe(checker.getTypeOfSymbol(call[0].getParameters()[0])),
      callReturn: call[0] && describe(checker.getReturnTypeOfSignature(call[0])),
      constructReturn: construct[0] && describe(checker.getReturnTypeOfSignature(construct[0])),
      properties: properties.map(property => ({ name: property.name, ...describe(checker.getTypeOfSymbol(property)) })) };
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}

test('ordinary reference graph shape is actual callable or constructable structure after canonical substitution', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [name, label, calls, constructs] of [
      ['object', 'Box<string>', 0, 0], ['callable', 'Callable<string>', 1, 0], ['constructable', 'Constructable<string>', 0, 1],
    ]) {
      const answer = await observe(name, config);
      assert.deepEqual(answer.before, { summary: label, errorType: false });
      assert.deepEqual(answer.after, answer.before);
      assert.equal(answer.calls, calls); assert.equal(answer.constructs, constructs);
      assert.deepEqual(answer.indexes, []);
      if (calls) {
        assert.deepEqual(answer.callParameter, { summary: 'string', errorType: false });
        assert.deepEqual(answer.callReturn, answer.callParameter);
      }
      if (constructs) assert.deepEqual(answer.constructReturn, { summary: 'string', errorType: false });
      if (name === 'object') assert.deepEqual(answer.properties, [{ name: 'value', summary: 'string', errorType: false }]);
    }
  }
});

test('required shape inventory leaves property values lazy but consumes actual index annotation recovery', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    const property = await observe('missingProperty', config);
    assert.deepEqual(property.before, { summary: 'MissingProperty<string>', errorType: false });
    assert.equal(property.calls, 0); assert.equal(property.constructs, 0);
    assert.deepEqual(property.properties, [{ name: 'value', summary: 'MissingType', errorType: true }, { name: 'actual', summary: 'string', errorType: false }]);
    const index = await observe('missingIndex', config);
    assert.deepEqual(index.before, { summary: 'MissingIndex<string>', errorType: false });
    assert.equal(index.calls, 1);
    assert.deepEqual(index.indexes, [{ summary: 'MissingType', errorType: true }]);
    assert.deepEqual(index.after, index.before);
    const conditional = await observe('conditionalIndex', config);
    assert.equal(conditional.calls, 1);
    assert.deepEqual(conditional.indexes, [{ summary: 'number', errorType: false }]);
    assert.deepEqual(conditional.after, { summary: 'ConditionalIndex<string>', errorType: false });
  }
});

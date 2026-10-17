import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-member-tables/', import.meta.url));

async function fresh(action, file = 'members.ts') {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  const config = path.join(root, 'tsconfig.json');
  const api = new native.API({ tsserverPath: native.binary, cwd: root });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config);
    assert.deepEqual([...project.program.getConfigFileParsingDiagnostics(),
      ...project.program.getProgramDiagnostics(), ...project.program.getSyntacticDiagnostics(),
      ...project.program.getBindDiagnostics()], []);
    const source = project.program.getSourceFile(path.join(root, file));
    await action(native, project.checker, source);
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}

test('native member declarations bind one compatible symbol across roles and merged declarations', async () => {
  await fresh((native, checker, source) => {
    const [first, second, class_] = source.statements;
    const symbol = node => checker.getSymbolAtLocation(node.name);
    const parameter = symbol(first.typeParameters[0]);
    const sameNamedProperty = symbol(first.members[1]);
    assert.equal(parameter.id, sameNamedProperty.id);
    assert.equal(parameter.id, symbol(second.typeParameters[0]).id);
    assert.equal(parameter.flags, native.SymbolFlags.TypeParameter | native.SymbolFlags.Property);
    assert.deepEqual(parameter.declarations.map(declaration => declaration.kind),
      [native.SyntaxKind.TypeParameter, native.SyntaxKind.PropertySignature, native.SyntaxKind.TypeParameter]);
    assert.equal(checker.typeToString(checker.getDeclaredTypeOfSymbol(parameter)), 'T');
    assert.equal(symbol(first.members[0]).id, symbol(second.members[0]).id);
    assert.deepEqual(symbol(first.members[0]).declarations.map(declaration => declaration.kind),
      [native.SyntaxKind.PropertySignature, native.SyntaxKind.PropertySignature]);
    assert.equal(symbol(first.members[8]).id, symbol(first.members[9]).id);
    assert.equal(symbol(first.members[10]).id, symbol(first.members[11]).id);
    assert.equal(symbol(first.members[10]).flags, native.SymbolFlags.GetAccessor | native.SymbolFlags.SetAccessor);
    assert.notEqual(symbol(class_.members[0]).id, symbol(class_.members[1]).id);
    assert.ok(symbol(class_.members[2]).name.endsWith('@#private'));
    assert.equal(symbol(class_.members[4]).flags, native.SymbolFlags.GetAccessor | native.SymbolFlags.SetAccessor);
    assert.equal(symbol(class_.members[5]).id, symbol(class_.members[6]).id);
  });
});

test('native member inventory retains exact literal keys and sorts by actual first declaration', async () => {
  await fresh((native, checker, source) => {
    const marker = '/*#query*/';
    const token = native.getTokenAtPosition(source, source.text.indexOf(marker) + marker.length);
    assert.equal(token.parent.kind, native.SyntaxKind.TypeReference);
    const type = checker.getTypeAtLocation(token.parent);
    assert.equal(type.flags, native.TypeFlags.Object);
    assert.equal(type.isErrorType(), false);
    const properties = checker.getPropertiesOfType(type);
    assert.deepEqual(properties.map(symbol => symbol.name),
      ['T', 'value', '', 'a.b', '16', 'computed', '17', 'escaped', 'run', 'paired', 'later']);
    assert.deepEqual(properties.map(symbol => checker.typeToString(checker.getTypeOfSymbolAtLocation(symbol, token))),
      ['number', 'number', 'string', 'number', 'number', 'string', 'number', 'string',
        '{ (x: number): number; (x: number, y: number): number; }', 'string', 'number']);
  });
});

test('native merged property value questions retain first declaration precedence without a diagnostic prelude', async () => {
  await fresh((native, checker, source) => {
    for (const [marker, summary] of [['numberFirst', 'number'], ['numberSecond', 'number'],
      ['stringFirst', 'string'], ['stringSecond', 'string']]) {
      const prefix = `/*#${marker}*/`;
      const token = native.getTokenAtPosition(source, source.text.indexOf(prefix) + prefix.length);
      assert.equal(token.parent.kind, native.SyntaxKind.PropertySignature);
      const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(token));
      assert.equal(type.isErrorType(), false, marker);
      assert.equal(type.flags, summary === 'number' ? native.TypeFlags.Number : native.TypeFlags.String, marker);
      assert.equal(checker.typeToString(type), summary, marker);
    }
  });
});


test('native merged generic member inventory orders a shared parameter/property by its first own declaration', async () => {
  await fresh((native, checker, source) => {
    const marker = '/*#inventory*/';
    const token = native.getTokenAtPosition(source, source.text.indexOf(marker) + marker.length);
    assert.equal(token.parent.kind, native.SyntaxKind.TypeReference);
    const type = checker.getTypeAtLocation(token.parent);
    assert.equal(type.flags, native.TypeFlags.Object);
    assert.equal(type.isErrorType(), false);
    assert.equal(checker.typeToString(type), 'NativeInventory<number>');
    const properties = checker.getPropertiesOfType(type);
    assert.deepEqual(properties.map(symbol => symbol.name), ['T', 'value', '', 'a.b', '16', 'escaped', 'later']);
    assert.deepEqual(properties.map(symbol => checker.typeToString(checker.getTypeOfSymbolAtLocation(symbol, token))),
      ['string', 'number', 'string', 'number', 'boolean', 'number', 'boolean']);
    assert.deepEqual(properties.map(symbol => symbol.declarations.length), [3, 2, 1, 1, 1, 1, 1]);
    const [first, second] = source.statements;
    const canonical = checker.getSymbolAtLocation(first.typeParameters[0].name);
    assert.equal(canonical.id, checker.getSymbolAtLocation(first.members[1].name).id);
    assert.equal(canonical.id, checker.getSymbolAtLocation(second.typeParameters[0].name).id);
    assert.equal(checker.typeToString(checker.getDeclaredTypeOfSymbol(canonical)), 'T');
    assert.equal(checker.typeToString(checker.getTypeOfSymbol(canonical)), 'string');
  }, 'inventory.ts');
});

test('native computed member-name operands keep their expression question', async () => {
  await fresh((native, checker, source) => {
    const offset = source.text.indexOf('["computed"]') + 1;
    const token = native.getTokenAtPosition(source, offset);
    assert.equal(token.kind, native.SyntaxKind.StringLiteral);
    assert.equal(token.parent.kind, native.SyntaxKind.ComputedPropertyName);
    const type = checker.getTypeAtLocation(token);
    assert.equal(type.flags, native.TypeFlags.StringLiteral);
    assert.equal(type.isErrorType(), false);
    assert.equal(checker.typeToString(type), '"computed"');
  });
});

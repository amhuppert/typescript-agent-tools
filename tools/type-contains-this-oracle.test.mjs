import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-contains-this/', import.meta.url));
const config = path.join(root, 'tsconfig.json');
async function fresh(action) {
  const native = await loadSemanticApi();
  const api = new native.API({ tsserverPath: native.binary, cwd: root });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config);
    assert.deepEqual([...project.program.getConfigFileParsingDiagnostics(),
      ...project.program.getProgramDiagnostics(), ...project.program.getSyntacticDiagnostics(),
      ...project.program.getBindDiagnostics()], []);
    await action(native, project);
  } finally {
    try { snapshot?.dispose(); } finally { api.close(); }
  }
}

function walk(node, action) {
  action(node);
  node.forEachChild(child => { walk(child, action); });
}

test('native binding finalizes ContainsThis with save/reset/propagate container semantics', async () => {
  await fresh((native, project) => {
    const source = project.program.getSourceFile(path.join(root, 'containers.ts'));
    assert.ok(source.flags & native.NodeFlags.ContainsThis, 'source owns its top-level this occurrence');
    const expected = new Map([
      ['Plain', false], ['NamedThis', false], ['Direct', true], ['Method', true],
      ['Call', true], ['Construct', true], ['FunctionProperty', true], ['ConstructorProperty', true],
      ['NestedType', true], ['Index', true], ['Generic', true], ['FunctionExpression', false],
      ['ArrowExpression', true], ['ObjectMethod', false], ['ClassMethod', false],
      ['ClassInitializer', false], ['EarlierThis', true], ['Inner', true],
      ['AfterFunction', false], ['NamespaceInner', true], ['AfterNamespace', false],
    ]);
    const observed = new Map();
    walk(source, node => {
      if (node.kind === native.SyntaxKind.InterfaceDeclaration) {
        observed.set(node.name.text, Boolean(node.flags & native.NodeFlags.ContainsThis));
      }
    });
    assert.deepEqual(observed, expected);

  });
});

test('ThisType demand distinguishes a real synthetic parameter from contextual native errorType', async () => {
  await fresh((native, project) => {
    const checker = project.checker;
    const source = project.program.getSourceFile(path.join(root, 'containers.ts'));
    const legal = new Set(['Direct', 'Method', 'Call', 'Construct', 'FunctionProperty',
      'ConstructorProperty', 'Index', 'EarlierThis', 'Inner', 'NamespaceInner']);
    let count = 0;
    walk(source, node => {
      if (node.kind !== native.SyntaxKind.ThisType) return;
      let declaration = node.parent;
      while (declaration.kind !== native.SyntaxKind.InterfaceDeclaration) declaration = declaration.parent;
      const type = checker.getTypeAtLocation(node);
      if (legal.has(declaration.name.text)) {
        assert.equal(type.flags, native.TypeFlags.TypeParameter);
        assert.equal(type.isErrorType(), false);
        assert.equal(checker.typeToString(type), 'this');
        const constraint = checker.getConstraintOfTypeParameter(type);
        assert.equal(constraint.flags, native.TypeFlags.Object);
        assert.equal(constraint.isErrorType(), false);
        assert.equal(checker.typeToString(constraint), declaration.name.text);
      } else {
        assert.equal(type.flags, native.TypeFlags.Any);
        assert.equal(type.isErrorType(), true);
      }
      count++;
    });
    assert.equal(count, 12);
    const contexts = project.program.getSourceFile(path.join(root, 'type-contexts.ts'));
    for (const [marker, valid] of [
      ['invalidAlias', false], ['invalidNested', false], ['invalidConstraint', false],
      ['invalidStatic', false], ['invalidConstructor', false], ['invalidFunction', false],
      ['validClass', true], ['validBody', true], ['validArrow', true],
      ['invalidStaticArrow', false], ['invalidNestedFunction', false], ['invalidComputed', false],
      ['validMergedOne', true], ['validMergedTwo', true],
    ]) {
      const prefix = `/*#${marker}*/`;
      const token = native.getTokenAtPosition(contexts, contexts.text.indexOf(prefix) + prefix.length);
      assert.equal(token.kind, native.SyntaxKind.ThisKeyword);
      const node = token.parent;
      assert.equal(node.kind, native.SyntaxKind.ThisType);
      const type = checker.getTypeAtLocation(node);
      assert.equal(type.flags, valid ? native.TypeFlags.TypeParameter : native.TypeFlags.Any, marker);
      assert.equal(type.isErrorType(), !valid, marker);
      assert.equal(checker.typeToString(type), valid ? 'this' : 'any', marker);
    }
    const merged = ['validMergedOne', 'validMergedTwo'].map(marker => {
      const prefix = `/*#${marker}*/`;
      return checker.getTypeAtLocation(native.getTokenAtPosition(contexts, contexts.text.indexOf(prefix) + prefix.length).parent);
    });
    assert.equal(merged[0].id, merged[1].id, 'one canonical merged synthetic-this identity');
  });
});

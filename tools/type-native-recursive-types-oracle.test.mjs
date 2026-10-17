// Native identities are compared inside one fresh checker, never across requests.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi, lineStarts, position } from './semantic-oracle.mjs';
import { typeOracle } from './type-oracle.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-recursive-types/', import.meta.url));
const config = fs.realpathSync(path.join(root, 'tsconfig.json'));
async function withChecker(action) {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  const api = new native.API({ tsserverPath: native.binary, cwd: root });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config);
    assert.ok(project, 'pinned compiler opens the fixture');
    for (const diagnostics of [project.program.getConfigFileParsingDiagnostics(), project.program.getProgramDiagnostics(), project.program.getSyntacticDiagnostics(), project.program.getBindDiagnostics()])
      assert.deepEqual(diagnostics, []);
    const source = project.program.getSourceFile(path.join(root, 'types.ts'));
    assert.ok(source);
    const declared = name => {
      const node = source.statements.find(node => node.kind === native.SyntaxKind.InterfaceDeclaration && node.name.text === name);
      assert.ok(node, name);
      const symbol = project.checker.getSymbolAtLocation(node.name);
      assert.ok(symbol);
      const type = project.checker.getDeclaredTypeOfSymbol(symbol);
      assert.equal(type.isErrorType(), false);
      return type;
    };
    const read = (type, name) => {
      const property = project.checker.getPropertiesOfType(type).find(property => property.name === name);
      assert.ok(property, name);
      const value = project.checker.getTypeOfSymbol(property);
      assert.equal(value.isErrorType(), false);
      return value;
    };
    return action({ declared, read });
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}

test('self-reference reuses its exact declared root identity', async () => {
  await withChecker(({ declared, read }) => {
    const recursive = declared('Recursive');
    assert.equal(read(recursive, 'next').id, recursive.id);
    assert.notEqual(read(recursive, 'value').id, recursive.id);
  });
});

test('equal shapes keep distinct nominal roots while repeated exact symbols share', async () => {
  await withChecker(({ declared, read }) => {
    const holder = declared('Holder');
    const first = read(holder, 'first'), second = read(holder, 'second'), again = read(holder, 'again');
    assert.notEqual(first.id, second.id);
    assert.equal(first.id, again.id);
    assert.equal(read(first, 'value').id, read(second, 'value').id);
    assert.equal(first.id, declared('Alpha').id);
    assert.equal(second.id, declared('Beta').id);
  });
});

test('cross-file renamed declaration questions preserve nominal summaries', async () => {
  const text = fs.readFileSync(path.join(root, 'consumer.ts'), 'utf8');
  for (const [marker, summary, properties] of [['ImportedRecursive', 'Recursive', [['next', 'Recursive'], ['value', 'string']]], ['ImportedHolder', 'Holder', [['first', 'Alpha'], ['second', 'Beta'], ['again', 'Alpha']]]]) {
    const answer = await typeOracle(config, { at: { file: 'consumer.ts', ...position(lineStarts(text), text.indexOf(marker)) } });
    assert.equal(answer.kind, 'declared');
    assert.equal(answer.summary, summary);
    assert.equal(answer.errorType, false);
    assert.deepEqual(answer.properties.map(property => [property.name, property.summary]), properties);
    assert.deepEqual(answer.prelude, []);
    assert.deepEqual(answer.diagnostics, []);
  }
});

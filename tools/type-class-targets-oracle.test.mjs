import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-class-targets/', import.meta.url));

// Compiler baseline only: fresh checking with the four input inventories,
// actual TypeReference question, and no semantic-diagnostic prelude.
async function observe(name, summary, action = () => {}) {
  const native = await loadSemanticApi();
  const directory = path.join(root, name);
  const config = path.join(directory, 'tsconfig.json');
  const api = new native.API({ tsserverPath: native.binary, cwd: directory });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config);
    const input = [...project.program.getConfigFileParsingDiagnostics(),
      ...project.program.getProgramDiagnostics(), ...project.program.getSyntacticDiagnostics(),
      ...project.program.getBindDiagnostics()];
    assert.deepEqual(input, []);
    const source = project.program.getSourceFile(path.join(directory, 'globals.ts'));
    const marker = '/*#query*/';
    const token = native.getTokenAtPosition(source, source.text.indexOf(marker) + marker.length);
    assert.equal(token.parent.kind, native.SyntaxKind.TypeReference);
    const checker = project.checker;
    const type = checker.getTypeAtLocation(token.parent);
    assert.equal(type.flags, native.TypeFlags.Object);
    assert.equal(type.isErrorType(), false);
    assert.equal(checker.typeToString(type), summary);
    const target = type.getTarget();
    assert.ok(target.getSymbol().flags & native.SymbolFlags.Class,
      'canonical Class flags retain precedence over interface/namespace attachments');
    const arguments_ = checker.getTypeArguments(type);
    const expected = name === 'nongeneric' ? [] : name === 'merged-default' ? ['string'] : ['number'];
    assert.deepEqual(arguments_.map(argument => checker.typeToString(argument)), expected);
    assert.ok(arguments_.every(argument => !argument.isErrorType()));
    return action({ project, checker, type, target });
  } finally {
    try { snapshot?.dispose(); } finally { api.close(); }
  }
}

test('canonical class references preserve generic defaults, merges and written names', async () => {
  for (const [name, summary] of [
    ['generic', 'Box<number>'], ['nongeneric', 'Box'], ['merged-default', 'Box<string>'],
    ['namespace-first-types', 'Box<number>'], ['interface-first', 'Box<number>'],
    ['escaped-first', '\\u0042ox<number>'], ['unused-constraint', 'Box<number>'],
    ['overridden-default', 'Box<number>'],
  ]) await observe(name, summary);
});

test('a healthy raw class type does not erase the separate namespace merge admission error', async () => {
  await observe('namespace-first', 'Box<number>', ({ project }) => {
    assert.deepEqual(project.program.getSemanticDiagnostics().map(diagnostic => diagnostic.code), [2434]);
  });
});

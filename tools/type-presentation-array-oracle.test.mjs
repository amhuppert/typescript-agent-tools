// These fresh pinned native observations authorize presentation demand
// boundaries; they do not claim native merged-member implementation.
import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/presentation-array/', import.meta.url));

async function observe(family, configName) {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  const directory = path.join(root, family);
  const config = path.join(directory, configName);
  const api = new native.API({ tsserverPath: native.binary, cwd: directory });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config);
    assert.ok(project, 'pinned project opens');
    for (const diagnostics of [project.program.getConfigFileParsingDiagnostics(), project.program.getProgramDiagnostics(),
      project.program.getSyntacticDiagnostics(), project.program.getBindDiagnostics()]) assert.deepEqual(diagnostics, []);
    const source = project.program.getSourceFile(path.join(directory, 'consumer.ts'));
    assert.ok(source);
    const token = native.getTokenAtPosition(source, source.text.indexOf('*/values') + 2);
    const type = project.checker.getTypeOfSymbol(project.checker.getSymbolAtLocation(token));
    const describe = value => ({ summary: project.checker.typeToString(value), errorType: value.isErrorType() });
    const before = { ...describe(type), isArray: project.checker.isArrayType(type),
      arguments: project.checker.getTypeArguments(type).map(describe) };
    const indexes = project.checker.getIndexInfosOfType(type).map(info => ({ key: describe(info.keyType),
      value: describe(info.valueType), readonly: info.isReadonly }));
    return { before, indexes, after: describe(type) };
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}

test('a healthy actual Array target retains a healthy summary across merged native member enumeration', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    const answer = await observe('merged', config);
    assert.deepEqual(answer.before, { summary: 'number[]', errorType: false, isArray: true,
      arguments: [{ summary: 'number', errorType: false }] });
    assert.deepEqual(answer.indexes, [{ key: { summary: 'number', errorType: false },
      value: { summary: 'number', errorType: false }, readonly: false }]);
    assert.deepEqual(answer.after, { summary: 'number[]', errorType: false });
  }
});

test('an unresolved index annotation is a consumed child error even though the Array root is non-error', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    const answer = await observe('invalid-index', config);
    assert.deepEqual(answer.before, { summary: 'number[]', errorType: false, isArray: true,
      arguments: [{ summary: 'number', errorType: false }] });
    assert.deepEqual(answer.indexes, [{ key: { summary: 'number', errorType: false },
      value: { summary: 'MissingType', errorType: true }, readonly: false }]);
    assert.deepEqual(answer.after, { summary: 'number[]', errorType: false });
  }
});

test('a native Array inventory can consume a site-sensitive type query after an actual guard join', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    const answer = await observe('flow-index', config);
    assert.deepEqual(answer.before, { summary: 'number[]', errorType: false, isArray: true,
      arguments: [{ summary: 'number', errorType: false }] });
    assert.deepEqual(answer.indexes, [{ key: { summary: 'number', errorType: false },
      value: { summary: 'string | number', errorType: false }, readonly: false }]);
    assert.deepEqual(answer.after, { summary: 'number[]', errorType: false });
  }
});

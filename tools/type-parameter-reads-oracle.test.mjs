import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-parameter-reads/', import.meta.url));
test('native parameter symbol and signature read share the actual returned identity', async () => {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  for (const [name, summary] of [
    ['shared', '(value: { n: number; }) => { n: number; }'],
    ['optional', '(value?: number | undefined) => number | undefined'],
    ['initialized', '(value?: number) => number'],
  ]) {
    const config = path.join(root, 'tsconfig.json');
    const api = new native.API({tsserverPath: native.binary, cwd: root});
    let snapshot;
    try {
      snapshot = api.updateSnapshot({openProject: config});
      const project = snapshot.getProject(config), checker = project.checker;
      assert.deepEqual([...project.program.getConfigFileParsingDiagnostics(),
        ...project.program.getProgramDiagnostics(), ...project.program.getSyntacticDiagnostics(),
        ...project.program.getBindDiagnostics()].map(d => d.code), []);
      const source = project.program.getSourceFile(path.join(root, 'parameters.ts'));
      const declaration = source.statements.find(declaration => declaration.name.text === name);
      const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(declaration.name));
      const signature = checker.getSignaturesOfType(type, native.SignatureKind.Call)[0];
      const parameter = checker.getTypeOfSymbol(signature.getParameters()[0]);
      const returned = checker.getReturnTypeOfSignature(signature);
      assert.equal(parameter.isErrorType(), false);
      assert.equal(returned.isErrorType(), false);
      assert.equal(parameter.id, returned.id, name);
      assert.equal(checker.typeToString(type), summary, name);
      if (name === 'initialized') assert.equal(parameter.flags, native.TypeFlags.Number);
    } finally { try {snapshot?.dispose();} finally {api.close();} }
  }
});

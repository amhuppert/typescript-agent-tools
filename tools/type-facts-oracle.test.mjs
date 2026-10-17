import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/facts-core/', import.meta.url));
for (const strict of [false, true]) test(`native unary union facts preserve truthy/falsy masks (strict=${strict})`, async () => {
  const native = await loadSemanticApi();
  const config = path.join(root, strict ? 'tsconfig.json' : 'loose.json');
  for (const [name, expected] of [['negatedLiteral', 'boolean'], ['negatedTruthy', strict ? 'false' : 'boolean'],
    ['negatedPrimitive', 'boolean'], ['negatedNullable', 'true'], ['negatedMixed', 'boolean'], ['negatedNever', 'boolean']]) {
    const api = new native.API({ tsserverPath: native.binary, cwd: root });
    let snapshot;
    try {
      snapshot = api.updateSnapshot({ openProject: config });
      const project = snapshot.getProject(config);
      for (const rows of [project.program.getConfigFileParsingDiagnostics(), project.program.getProgramDiagnostics(),
        project.program.getSyntacticDiagnostics(), project.program.getBindDiagnostics()]) assert.deepEqual(rows, []);
      const source = project.program.getSourceFile(path.join(root, 'cases.ts'));
      const node = native.getTokenAtPosition(source, source.text.indexOf(name));
      const type = project.checker.getTypeOfSymbol(project.checker.getSymbolAtLocation(node));
      assert.equal(type.isErrorType(), false);
      assert.equal(project.checker.typeToString(type), expected);
    } finally { try { snapshot?.dispose(); } finally { api.close(); } }
  }
});

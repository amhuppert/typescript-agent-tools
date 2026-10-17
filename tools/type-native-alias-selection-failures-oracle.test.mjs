import assert from 'node:assert/strict';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-alias-selection/', import.meta.url));
test('missing and cyclic alias targets are unknown before Q4 meaning selection', async () => {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  const config = path.join(root, 'broken.json');
  for (const names of [['missing', 'cycle', 'healthy'], ['healthy', 'cycle', 'missing']]) {
    const api = new native.API({tsserverPath: native.binary, cwd: root});
    let snapshot;
    try {
      snapshot = api.updateSnapshot({openProject: config});
      const project = snapshot.getProject(config);
      const source = project.program.getSourceFile(path.join(root, 'broken.ts'));
      for (const name of names) {
        const marker = `/*#${name}*/`;
        const token = native.getTokenAtPosition(source, source.text.indexOf(marker) + marker.length);
        const alias = project.checker.getSymbolAtLocation(token);
        const target = project.checker.getAliasedSymbol(alias);
        assert.equal(project.checker.isUnknownSymbol(target), name !== 'healthy', name);
        if (name === 'healthy') {
          const type = project.checker.getTypeOfSymbol(target);
          assert.equal(project.checker.typeToString(type), '"fixed"');
          assert.equal(type.isErrorType(), false);
        }
      }
    } finally { try { snapshot?.dispose(); } finally { api.close(); } }
  }
});

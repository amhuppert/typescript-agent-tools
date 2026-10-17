import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-type-order-exports/', import.meta.url));
test('canonical type symbols distinguish default exports, ordinary exports and written names', async () => {
  const native = await loadSemanticApi();
  const api = new native.API({tsserverPath: native.binary, cwd: root});
  let snapshot;
  try {
    snapshot = api.updateSnapshot({openProject: path.join(root, 'tsconfig.json')});
    const project = snapshot.getProject(path.join(root, 'tsconfig.json'));
    const checker = project.checker;
    assert.deepEqual([
      project.program.getConfigFileParsingDiagnostics(), project.program.getProgramDiagnostics(),
      project.program.getSyntacticDiagnostics(), project.program.getBindDiagnostics(),
    ].map(rows => rows.map(row => row.code)), [[], [], [], []]);
    for (const [filename, marker, summary, names] of [
      ['default.ts', 'local', 'Zebra | Alpha', ['Zebra', 'default']],
      ['imported.ts', 'imported', 'Zebra | Alpha', ['Zebra', 'default']],
      ['normal.ts', 'normal', 'Alpha | Zebra', ['Alpha', 'Zebra']],
      ['escaped.ts', 'escaped', 'Alpha | \\u005aebra', ['Alpha', 'Zebra']],
      ['escaped.ts', 'parameters', 'Alpha | Zebra', ['Alpha', 'Zebra']],
    ]) {
      const text = fs.readFileSync(path.join(root, filename), 'utf8');
      const tag = `/*#${marker}*/`;
      const file = project.program.getSourceFile(path.join(root, filename));
      const node = native.getTokenAtPosition(file, text.indexOf(tag) + tag.length);
      const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(node));
      assert.equal(type.isErrorType(), false, marker);
      assert.equal(checker.typeToString(type), summary, marker);
      assert.deepEqual(type.getTypes().map(child => child.getSymbol().name), names, marker);
    }
    const file = project.program.getSourceFile(path.join(root, 'default.ts'));
    const declaration = file.statements[0];
    const selected = checker.getSymbolAtLocation(declaration.name);
    const target = checker.getDeclaredTypeOfSymbol(selected);
    assert.equal(target.getSymbol().name, 'default');
    assert.equal(selected.name, 'default');
  } finally {
    try { snapshot?.dispose(); } finally { api.close(); }
  }
});

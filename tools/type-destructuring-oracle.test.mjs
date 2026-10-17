import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle, compilerHookOracle} from './type-oracle.mjs';
import {loadSemanticApi, lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-destructuring/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const config of ['tsconfig.json', 'loose.json', 'unchecked.json']) {
  test(`native destructured assignment producer controls (${config})`, async () => {
    const controls = [...source.matchAll(/\/\*#(\w+)\*\//g)];
    assert.equal(controls.length, 11);
    for (const match of controls) {
      const answer = await typeOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), match.index + match[0].length)},
        properties: false,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.history, 'baseline');
      assert.equal(answer.kind, 'expression', match[1]);
      assert.equal(answer.summary, 'string | number', match[1]);
      assert.equal(answer.errorType, false, match[1]);
      assert.deepEqual(answer.prelude, []);
      // Separate fresh checkers establish identity and demand history. Neither
      // a display string nor a non-error root proves that an unresolved sibling
      // was skipped by the actual assignment operation.
      const native = await loadSemanticApi();
      const api = new native.API({tsserverPath: native.binary, cwd: root});
      let snapshot;
      try {
        snapshot = api.updateSnapshot({openProject: path.join(root, config)});
        const project = snapshot.getProject(path.join(root, config));
        for (const diagnostics of [project.program.getConfigFileParsingDiagnostics(),
          project.program.getProgramDiagnostics(), project.program.getSyntacticDiagnostics(),
          project.program.getBindDiagnostics()]) assert.deepEqual(diagnostics, []);
        const file = project.program.getSourceFile(path.join(root, 'cases.ts'));
        const node = native.getTokenAtPosition(file, match.index + match[0].length);
        const actual = project.checker.getTypeAtLocation(node);
        const declared = project.checker.getTypeOfSymbol(project.checker.getSymbolAtLocation(node));
        assert.equal(actual.id, declared.id, `${config}:${match[1]}: exact declared identity`);
      } finally {
        try { snapshot?.dispose(); } finally { api.close(); }
      }
      const phase = await compilerHookOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), match.index + match[0].length)},
        operation: 'type',
      });
      assert.equal(phase.summary, 'string | number');
      assert.ok(!phase.beforeSummary.diagnosticCodes.includes(2304),
        `${config}:${match[1]}: unresolved sibling must remain unvisited`);
    }
  });
}

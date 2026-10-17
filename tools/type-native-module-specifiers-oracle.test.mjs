import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position, loadSemanticApi} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-module-specifiers/', import.meta.url));
for (const profile of ['bundler', 'no-package-modes', 'node']) {
  test(`native contextual module specifier and resolution modes (${profile})`, async () => {
    const rows = [];
    for (const file of ['reuse.ts', 'first-require.mts', 'first-import.mts', 'first-import.cts',
      'first-require.cts', 'dynamic.ts', 'import-type.ts', 'require.mts',
      'missing-side.ts', 'javascript.ts', 'unadmitted-tsx.ts', 'helpers.ts']) {
      const source = fs.readFileSync(path.join(root, file), 'utf8');
      const marker = '/*#module*/';
      const answer = await compilerHookOracle(path.join(root, `${profile}-${file}.json`), {
        history: 'cold', inspectSymbolDisplay: true, symbolDisplayEnclosing: true,
        at: {file, ...position(lineStarts(source), source.indexOf(marker) + marker.length)},
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.deepEqual(answer.beforeSymbolDisplay.diagnosticCodes, []);
      assert.deepEqual(answer.diagnosticCodes, []);
      assert.deepEqual(answer.demandFailures, []);
      // Ask alias selection independently: diagnostic-only module display
      // must finish before the missing target becomes native unknownSymbol.
      const native = await loadSemanticApi();
      const api = new native.API({tsserverPath: native.binary, cwd: root});
      let snapshot;
      try {
        const config = path.join(root, `${profile}-${file}.json`);
        snapshot = api.updateSnapshot({openProject: config});
        const project = snapshot.getProject(config);
        const input = project.program.getSourceFile(path.join(root, file));
        const missing = '/*#missing*/';
        const token = native.getTokenAtPosition(input, source.indexOf(missing) + missing.length);
        const symbol = project.checker.getSymbolAtLocation(token);
        assert.equal(project.checker.isUnknownSymbol(project.checker.getAliasedSymbol(symbol)), true);
      } finally { try { snapshot?.dispose(); } finally { api.close(); } }
      rows.push({file, display: Buffer.from(answer.symbolDisplayBytes, 'base64').toString('utf8'),
        defaultMode: answer.defaultResolutionMode, resolvedPath: answer.selectedModuleResolvedPath, imports: answer.importResolutionModes});
    }
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-module-specifiers-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

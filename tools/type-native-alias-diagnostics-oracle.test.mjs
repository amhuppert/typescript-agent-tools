import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-alias-diagnostics/', import.meta.url));
for (const profile of ['strict', 'no-check']) {
  test(`native missing alias diagnostic branches (${profile})`, async () => {
    const native = await loadSemanticApi();
    assert.equal(native.version, '7.0.2');
    const config = path.join(root, `${profile}.json`);
    const rows = [];
    for (const reverse of [false, true]) {
      const api = new native.API({tsserverPath: native.binary, cwd: root});
      let snapshot;
      try {
        snapshot = api.updateSnapshot({openProject: config});
        const project = snapshot.getProject(config);
        const source = project.program.getSourceFile(path.join(root, 'cases.ts'));
        const markers = [...source.text.matchAll(/\/\*#([^*]+)\*\//g)];
        if (reverse) markers.reverse();
        const results = [];
        for (const marker of markers) {
          const token = native.getTokenAtPosition(source, marker.index + marker[0].length);
          const alias = project.checker.getSymbolAtLocation(token);
          const target = project.checker.getAliasedSymbol(alias);
          const unknown = project.checker.isUnknownSymbol(target);
          assert.equal(unknown, marker[1] !== 'healthy', marker[1]);
          results.push({name: marker[1], unknown, ...(unknown ? {} : {
            summary: project.checker.typeToString(project.checker.getTypeOfSymbol(target)),
          })});
        }
        if (reverse) results.reverse();
        if (reverse) assert.deepEqual(results, rows);
        else rows.push(...results);
        // Whole-program diagnostics confirm the fixture branches. They are
        // separate from the query sequence and are not demand-trace evidence.
        const codes = project.program.getSemanticDiagnostics().map(d => d.code).sort((a, b) => a - b);
        if (!reverse) {
          fs.mkdirSync('.cc/temp', {recursive: true});
          fs.writeFileSync(`.cc/temp/native-alias-diagnostics-${profile}.json`, JSON.stringify({rows, codes}, null, 2) + '\n');
          assert.deepEqual({rows, codes}, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
        }
      } finally { try { snapshot?.dispose(); } finally { api.close(); } }
    }
    assert.equal(rows.length, 20);
  });
}

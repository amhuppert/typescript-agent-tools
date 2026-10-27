import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-symbol-location/', import.meta.url));
test('symbol-location syntax evidence preserves nil and proves the actual variable header', async () => {
  const native = await loadSemanticApi(), config = path.join(root, 'tsconfig.json');
  const api = new native.API({tsserverPath: native.binary, cwd: root});
  let snapshot;
  try {
    snapshot = api.updateSnapshot({openProject: config});
    const project = snapshot.getProject(config);
    const rows = [];
    for (const name of ['cases.ts', 'merge-read.ts']) {
    const file = project.program.getSourceFile(path.join(root, name));
    const source = file.text;
    for (const marker of source.matchAll(/\/\*@([^*]+)\*\//g)) {
      const token = native.getTokenAtPosition(file, marker.index + marker[0].length);
      // Native API token lookup synthesizes unparented keyword tokens; those
      // are not RemoteNodes and cannot be submitted to this checker protocol.
      // Their source-backed dispatch review is distinct from these observations.
      if (marker[1].endsWith('Keyword')) continue;
      let symbol;
      try { symbol = project.checker.getSymbolAtLocation(token); }
      catch (error) { throw new Error(`${marker[1]} (${native.SyntaxKind[token.kind]}): ${error.message}`, {cause: error}); }
      rows.push({file: name, name: marker[1], syntax: native.SyntaxKind[token.kind], flags: symbol?.flags ?? null,
        declarations: symbol?.declarations.map(handle => {
          const node = handle.resolve(project);
          assert.ok(node);
          return {file: path.basename(handle.path), kind: native.SyntaxKind[node.kind], start: node.pos, end: node.end};
        }) ?? []});
    }
    }
    fs.writeFileSync(scratchPath('native-symbol-location-symbols.json'), JSON.stringify(rows, null, 2) + '\n');
    for (const name of ['ordinaryLiteral', 'surrogateLiteral', 'surrogateTemplate'])
      assert.equal(rows.find(row => row.name === name).flags, null, name);
    for (const name of ['transientVariable', 'transientClosure']) {
      const row = rows.find(row => row.name === name);
      assert.ok(row.flags & native.SymbolFlags.Transient, `${name}: transient`);
      assert.ok(row.flags & native.SymbolFlags.Variable, `${name}: variable`);
      assert.equal(row.declarations.length, 2, `${name}: actual merged declarations`);
    }
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, 'expected-symbols.json'), 'utf8')));
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
});

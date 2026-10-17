import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position, loadSemanticApi} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-combined-alias/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const profile of ['strict', 'loose']) {
  test(`combined value/type alias target (${profile})`, async () => {
    const rows = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file: 'cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const config = path.join(root, `${profile}.json`);
      const native = await compilerHookOracle(config, {at});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      assert.equal(native.errorType, false, marker[1]);
      assert.deepEqual(native.beforeSummary.demandFailures, [], marker[1]);
      rows.push({name: marker[1], kind: api.kind, location: api.location, summary: native.summary,
        errorType: native.errorType, codes: native.beforeSummary.diagnosticCodes,
        demandFailures: native.beforeSummary.demandFailures});
    }
    assert.equal(rows.length, 13);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-combined-alias-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

test('separate combined alias factories retain distinct identities with the same declarations', async () => {
  const native = await loadSemanticApi();
  const config = path.join(root, 'strict.json');
  const api = new native.API({tsserverPath: native.binary, cwd: root});
  let snapshot;
  try {
    snapshot = api.updateSnapshot({openProject: config});
    const project = snapshot.getProject(config);
    const targets = [];
    for (const file of ['same-a.ts', 'same-b.ts']) {
      const source = project.program.getSourceFile(path.join(root, file));
      const token = native.getTokenAtPosition(source, source.text.indexOf('SharedCombined'));
      const target = project.checker.getAliasedSymbol(project.checker.getSymbolAtLocation(token));
      assert.ok(target.flags & native.SymbolFlags.Transient);
      assert.ok(target.flags & native.SymbolFlags.Property);
      assert.ok(target.flags & native.SymbolFlags.Interface);
      targets.push(target);
    }
    assert.notEqual(targets[0].id, targets[1].id);
    const declarations = symbol => symbol.declarations.map(handle => {
      const node = handle.resolve(project);
      assert.ok(node);
      return [handle.path, node.kind, node.pos, node.end];
    });
    assert.deepEqual(declarations(targets[0]), declarations(targets[1]));
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
});

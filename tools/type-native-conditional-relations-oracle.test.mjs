import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position, loadSemanticApi} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-conditional-relations/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');

// Observe the actual merged declaration identity independently of type queries.
// Numeric symbol IDs are request-local and are not part of the evidence format.
async function actualAlias(config) {
  const native = await loadSemanticApi();
  const api = new native.API({tsserverPath: native.binary, cwd: root});
  let snapshot;
  try {
    snapshot = api.updateSnapshot({openProject: config});
    const project = snapshot.getProject(config);
    const file = project.program.getSourceFile(path.join(root, 'cases.ts'));
    const marker = '/*@createdAliasSymbol*/';
    const token = native.getTokenAtPosition(file, source.indexOf(marker) + marker.length);
    const symbol = project.checker.getSymbolAtLocation(token);
    assert.ok(symbol);
    const declarations = symbol.declarations.map(handle => {
      const node = handle.resolve(project);
      assert.ok(node);
      return {kind: native.SyntaxKind[node.kind], pos: node.pos, end: node.end};
    });
    return {flags: symbol.flags, checkFlags: symbol.checkFlags,
      transient: !!(symbol.flags & native.SymbolFlags.Transient),
      property: !!(symbol.flags & native.SymbolFlags.Property),
      typeAlias: !!(symbol.flags & native.SymbolFlags.TypeAlias), declarations};
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}

for (const profile of ['strict', 'loose', 'nocheck']) {
  test(`native conditional relation and inference continuations (${profile})`, async () => {
    const config = path.join(root, `${profile}.json`);
    const alias = await actualAlias(config);
    assert.equal(alias.transient, true);
    assert.equal(alias.property, true);
    assert.equal(alias.typeAlias, true);
    const queries = [];
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file: 'cases.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const native = await compilerHookOracle(config, {at, history: 'cold',
        inspectDiagnosticCache: true, inspectExpressionAcquisitions: true});
      const api = await typeOracle(config, {at, history: 'cold', properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      const errors = (native.beforeSummary.displayAcquisitions ?? [])
        .filter(event => event.event === 'exit' && event.errorType)
        .map(({operation, pos, end, declarationPos, declarationEnd, name}) => ({operation, pos, end, declarationPos, declarationEnd, name}));
      queries.push({name: marker[1], summary: native.summary, errorType: native.errorType,
        codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures, errors});
      fs.writeFileSync(scratchPath(`native-conditional-relations-${profile}.json`), JSON.stringify({alias, queries}, null, 2) + '\n');
    }
    assert.equal(queries.length, 11);
    assert.deepEqual(JSON.parse(JSON.stringify({alias, queries})), JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position, loadSemanticApi} from './semantic-oracle.mjs';

const fixture = fileURLToPath(new URL('../fixtures/types/native-json-initialization/', import.meta.url));

// A separate cold API checker observes the actual alias target. In particular,
// cloning an augmented JSON property discards its original installed type and
// rechecks the copied first value declaration. Numeric symbol IDs are
// request-local and are deliberately absent from this portable evidence.
async function actualAlias(config, root, source) {
  const native = await loadSemanticApi();
  const api = new native.API({tsserverPath: native.binary, cwd: root});
  let snapshot;
  try {
    snapshot = api.updateSnapshot({openProject: config});
    const project = snapshot.getProject(config);
    const input = project.program.getSourceFile(path.join(root, 'main.ts'));
    const marker = '/*#alias*/';
    const token = native.getTokenAtPosition(input, source.indexOf(marker) + marker.length);
    const alias = project.checker.getSymbolAtLocation(token);
    assert.ok(alias.flags & native.SymbolFlags.Alias);
    const symbol = project.checker.getAliasedSymbol(alias);
    assert.equal(project.checker.isUnknownSymbol(symbol), false);
    const declaration = handle => {
      const node = handle.resolve(project);
      assert.ok(node);
      let file = node;
      while (file.parent) file = file.parent;
      return {file: path.relative(root, file.fileName).split(path.sep).join('/'),
        kind: native.SyntaxKind[node.kind], pos: node.pos, end: node.end};
    };
    const type = project.checker.getTypeOfSymbol(symbol);
    return {flags: symbol.flags, checkFlags: symbol.checkFlags,
      declarations: symbol.declarations.map(declaration),
      valueDeclaration: symbol.valueDeclaration ? declaration(symbol.valueDeclaration) : null,
      summary: project.checker.typeToString(type), errorType: type.isErrorType()};
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}

for (const profile of ['strict', 'loose']) {
  test(`native JSON initialization preserves actual property metadata and final queries (${profile})`, async () => {
    const cases = JSON.parse(fs.readFileSync(path.join(fixture, 'cases.json'), 'utf8'));
    const rows = [];
    for (const entry of cases) {
      const root = path.resolve('.cc/temp/native-json-initialization', profile, entry.name);
      fs.mkdirSync(root, {recursive: true});
      for (const [file, text] of Object.entries(entry.files)) fs.writeFileSync(path.join(root, file), text);
      const config = path.join(root, 'tsconfig.json');
      fs.copyFileSync(path.join(fixture, `${profile}.json`), config);
      const source = entry.files['main.ts'];
      const actual = await actualAlias(config, root, source);
      const queries = [];
      for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
        const at = {file: 'main.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
        const native = await compilerHookOracle(config, {at, history: 'cold',
          inspectDiagnosticCache: true, inspectExpressionAcquisitions: true});
        const api = await typeOracle(config, {at, history: 'cold', properties: false});
        assert.equal(native.compiler, '7.0.2');
        assert.equal(native.checkerCount, 1);
        assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], `${entry.name}/${marker[1]}`);
        const errors = (native.beforeSummary.displayAcquisitions ?? []).filter(event => event.event === 'exit' && event.errorType)
          .map(({operation, pos, end, declarationPos, declarationEnd, name}) => ({operation, pos, end, declarationPos, declarationEnd, name}));
        queries.push({name: marker[1], kind: api.kind, summary: native.summary, errorType: native.errorType,
          flags: native.symbolFlags, origins: api.origins, codes: native.beforeSummary.diagnosticCodes,
          demandFailures: native.beforeSummary.demandFailures, errors});
      }
      assert.equal(queries.length, 3);
      assert.equal(queries.find(row => row.name === 'alias').summary, actual.summary, entry.name);
      rows.push({name: entry.name, actual, queries});
      fs.writeFileSync(`.cc/temp/native-json-initialization-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    }
    assert.equal(rows.length, 12);
    assert.deepEqual(JSON.parse(JSON.stringify(rows)), JSON.parse(fs.readFileSync(path.join(fixture, `expected-${profile}.json`), 'utf8')));
  });
}

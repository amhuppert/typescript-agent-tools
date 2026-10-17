import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position, loadSemanticApi} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-string-values/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'switches.ts'), 'utf8');
for (const mode of ['strict', 'loose']) {
  test(`native exact typeof-switch witnesses (${mode})`, async () => {
    const rows = [];
    const config = path.join(root, `switches-${mode}.json`);
    for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file:'switches.ts', ...position(lineStarts(source), marker.index + marker[0].length)};
      const native = await compilerHookOracle(config, {at});
      const api = await typeOracle(config, {at, properties:false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary,native.errorType], [api.summary,api.errorType], marker[1]);
      assert.equal(native.errorType, false, marker[1]);
      assert.deepEqual(native.beforeSummary.demandFailures, [], marker[1]);
      rows.push({name:marker[1],summary:native.summary,errorType:native.errorType,location:api.location,
        origins:api.origins,codes:native.beforeSummary.diagnosticCodes,demandFailures:native.beforeSummary.demandFailures});
    }
    assert.equal(rows.length, 10);
    fs.mkdirSync('.cc/temp', {recursive:true});
    fs.writeFileSync(`.cc/temp/native-string-switches-${mode}.json`, JSON.stringify(rows,null,2)+'\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root,`expected-switches-${mode}.json`),'utf8')));
  });
}

test('native public AST preserves ordinary versus JSX string scanner modes', async () => {
  const semantic = await loadSemanticApi();
  const config = path.join(root,'switches-strict.json');
  const api = new semantic.API({tsserverPath:semantic.binary,cwd:root});
  let snapshot;
  try {
    snapshot = api.updateSnapshot({openProject:config});
    const project = snapshot.getProject(config);
    const source = project.program.getSourceFile(path.join(root,'dialects.tsx'));
    const declarations = source.statements.map(statement => statement.declarationList.declarations[0]);
    const ordinary = declarations[0].initializer;
    const attribute = declarations[1].initializer.attributes.properties[0].initializer;
    assert.equal(ordinary.text, 'A\n&amp;');
    assert.equal(attribute.text, '\\u0041\\n&amp;');
    assert.equal(ordinary.kind, semantic.SyntaxKind.StringLiteral);
    assert.equal(attribute.kind, semantic.SyntaxKind.StringLiteral);
    assert.deepEqual(project.program.getSyntacticDiagnostics(), []);
  } finally { try {snapshot?.dispose();} finally {api.close();} }
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';

test('native final AST survives unterminated JSDoc string and template literals', async () => {
  const rows = JSON.parse(fs.readFileSync(new URL('../fixtures/types/native-string-values/jsdoc-malformed.json',import.meta.url),'utf8'));
  assert.equal(rows.length,38);
  const semantic = await loadSemanticApi();
  const root = fs.mkdtempSync(path.join(os.tmpdir(),'native-string-jsdoc-'));
  let api, snapshot;
  try {
    const files = rows.map(row => `${row.name}.ts`);
    rows.forEach((row,index) => fs.writeFileSync(path.join(root,files[index]),row.source));
    const config = path.join(root,'tsconfig.json');
    fs.writeFileSync(config,JSON.stringify({compilerOptions:{noLib:true},files}));
    api = new semantic.API({tsserverPath:semantic.binary,cwd:root});
    snapshot = api.updateSnapshot({openProject:config});
    const {program} = snapshot.getProject(config);
    assert.deepEqual(program.getSyntacticDiagnostics(),[]);
    for (const [index,row] of rows.entries()) {
      const source = program.getSourceFile(path.join(root,files[index]));
      assert.equal(source.statements.length,1,row.name);
      const declaration = source.statements[0].declarationList.declarations[0];
      assert.equal(declaration.name.text,row.nameText,row.name);
      assert.equal(declaration.initializer.text,row.literalText,row.name);
    }
  } finally {
    try { snapshot?.dispose(); } finally { api?.close(); fs.rmSync(root,{recursive:true,force:true}); }
  }
});

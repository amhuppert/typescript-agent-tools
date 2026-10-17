import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-import-branch-resources');
test('native qualified, ambient namespace and global import branches',async()=>{
  const rows=[];
  for(const config of ['strict','nocheck','loose','loose-nocheck']){
    for(const [file,marker] of [['factory.ts','equalsMember'],['default.ts','defaultMember'],['ambient.d.ts','ambientMember'],['global-module.d.ts','globalExport']]){
      const source=fs.readFileSync(path.join(root,file),'utf8'),tag=`/*#${marker}*/`;
      const answer=await compilerHookOracle(path.join(root,`${config}.json`),{operation:'member',at:{file,...position(lineStarts(source),source.indexOf(tag)+tag.length)}});
      rows.push({config,file,marker,answer});
      fs.writeFileSync('.cc/temp/native-import-branch-resources.json',JSON.stringify(rows,null,2)+'\n');
      assert.equal(answer.compiler,'7.0.2');
      assert.equal(answer.checkerCount,1);
      assert.deepEqual(answer.demandFailures,[],`${config}/${marker}`);
      assert.deepEqual(answer.diagnosticCodes,[],`${config}/${marker}`);
      assert.ok(answer.groups.length>0,`${config}/${marker}`);
    }
  }
  assert.equal(rows.length,16);
});

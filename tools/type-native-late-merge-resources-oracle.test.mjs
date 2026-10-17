import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-late-merge-resources');
test('native flat late method merges retain the final numeric call result',async()=>{
  const rows=[];
  for(const config of ['strict','nocheck','loose','loose-nocheck']){
    for(const width of [16,64]){
      const file=`${width}.ts`,source=fs.readFileSync(path.join(root,file),'utf8');
      const answer=await compilerHookOracle(path.join(root,`${config}.json`),{
        history:'cold',inspectSymbolHeaders:true,
        at:{file,...position(lineStarts(source),source.indexOf('const chosen')+6)},
      });
      rows.push({config,width,file,answer});
      fs.mkdirSync('.cc/temp',{recursive:true});
      fs.writeFileSync('.cc/temp/native-late-merge-resources.json',JSON.stringify(rows,null,2)+'\n');
      assert.equal(answer.compiler,'7.0.2');
      assert.equal(answer.checkerCount,1);
      assert.equal(answer.summary,'number');
      assert.equal(answer.errorType,false);
      assert.deepEqual(answer.demandFailures,[]);
      assert.deepEqual(answer.beforeSummary.diagnosticCodes,[]);
    }
  }
  assert.equal(rows.length,8);
});

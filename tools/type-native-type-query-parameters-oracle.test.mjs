import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle, compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-type-query-parameters/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const markers=['local','external','qualifiedLocal','qualifiedExternal','arguments','imported','this','lazyHealthy','lazyFailure','missingHealthy','missingFailure'];
for(const config of ['tsconfig.json','exact.json','loose.json']){
 test(`native type-query outer parameter references (${config})`,async()=>{
  const observed=[];
  for(const marker of markers){
   const tag=`/*#${marker}*/`;
   const answer=await typeOracle(path.join(root,config),{at:{file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)},properties:false});
   assert.equal(answer.compiler,'7.0.2');
   observed.push({marker,summary:answer.summary,errorType:answer.errorType});
   if (['lazyHealthy','lazyFailure','missingHealthy','missingFailure'].includes(marker)) {
     const acquired = await compilerHookOracle(path.join(root,config), {at:{file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)}});
     assert.deepEqual([acquired.summary,acquired.errorType],[answer.summary,answer.errorType]);
     assert.ok(acquired.beforeSummary.diagnosticCodes.includes(2304), `${config}/${marker}: unresolved symbol was acquired`);
   }
  }
  fs.mkdirSync('.cc/temp',{recursive:true});
  fs.writeFileSync(`.cc/temp/native-type-query-parameters-${config}`,JSON.stringify(observed,null,2));
  const summaries=['string','"external"','string','"namespace"','(value: number) => number','"imported"','"this-healthy"','"healthy"','any','"missing-healthy"','any'];
  assert.deepEqual(observed,markers.map((marker,index)=>({marker,summary:summaries[index],errorType:marker==='lazyFailure'||marker==='missingFailure'})));
 });
}

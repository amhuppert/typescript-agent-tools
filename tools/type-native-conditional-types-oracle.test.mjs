import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-conditional-types/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const markers=['true','false','union','any','never','unknown','wrapped','wrappedNever','arrayInfer','repeatInfer','returnInfer','constrainedInfer','trueSubstitution','nested','tail','cross','nullable','deferred','skippedMissing','consumedMissing'];
for(const config of ['tsconfig.json','exact.json','loose.json']){
 test(`native conditional branches, inference and retained laziness (${config})`,async()=>{
  const observed=[];
  for(const marker of markers){
   const tag=`/*#${marker}*/`;
   const answer=await typeOracle(path.join(root,config),{at:{file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)},properties:false});
   assert.equal(answer.compiler,'7.0.2');
   observed.push({marker,summary:answer.summary,errorType:answer.errorType});
  }
  fs.mkdirSync('.cc/temp',{recursive:true});
  fs.writeFileSync(`.cc/temp/native-conditional-types-${config}`,JSON.stringify(observed,null,2));
  const summaries=['"yes"','"no"','"no" | "yes"','"no" | "yes"','never','"no"','"no"','"yes"','"a" | "b"','"a" | 1','"return"','"a"','number','"number" | "other"','"done"','"a"',config==='loose.json'?'"value"':'"nullish" | "value"','<T extends { a: unknown; }>() => Deferred<T>','"safe"','MissingConditional'];
  assert.deepEqual(observed,markers.map((marker,index)=>({marker,summary:summaries[index],errorType:marker==='consumedMissing'})));
 });
}

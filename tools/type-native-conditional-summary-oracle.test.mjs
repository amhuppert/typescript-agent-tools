import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-conditional-types/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const markers=['inlineArray','inlineConstrained','inlineReturn','inlineRest','inlineTemplate','inlineNested','inlineFunctionCheck','inlineTupleConstrained'];
for(const config of ['tsconfig.json','exact.json','loose.json']){
 test(`native inline conditional inference summaries (${config})`,async()=>{
  const observed=[];
  for(const marker of markers){
   const tag=`/*#${marker}*/`;
   const answer=await typeOracle(path.join(root,config),{at:{file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)},properties:false});
   assert.equal(answer.compiler,'7.0.2');
   observed.push({marker,summary:answer.summary,errorType:answer.errorType});
  }
  fs.mkdirSync('.cc/temp',{recursive:true});
  fs.writeFileSync(`.cc/temp/native-conditional-summary-${config}`,JSON.stringify(observed,null,2));
  assert.deepEqual(observed,[{"marker":"inlineArray","summary":"<T>() => T extends (infer U)[] ? U : never","errorType":false},{"marker":"inlineConstrained","summary":"<T>() => T extends infer U extends string ? U : never","errorType":false},{"marker":"inlineReturn","summary":"<T>() => T extends () => (infer U extends string) ? U : never","errorType":false},{"marker":"inlineRest","summary":"<T>() => T extends [unknown, ...infer R] ? R : never","errorType":false},{"marker":"inlineTemplate","summary":"<T>() => T extends `${infer U}` ? U : never","errorType":false},{"marker":"inlineNested","summary":"<T, V>() => T extends (V extends infer W ? W : never) ? T : never","errorType":false},{"marker":"inlineFunctionCheck","summary":"<T>() => T","errorType":false},{"marker":"inlineTupleConstrained","summary":"<T>() => T extends [infer U extends string] ? U : never","errorType":false}]);
 });
}

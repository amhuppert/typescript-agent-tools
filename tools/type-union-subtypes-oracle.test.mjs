import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-union-subtypes/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
for (const config of ['tsconfig.json','loose.json']) {
 test(`native subtype union reduction (${config})`,async()=>{
  const observed=[];
  for (const marker of ['base','reverse','discriminated','hierarchy','unrelated','empty','primitive','constrained','aliases']) {
   const tag=`/*#${marker}*/`;
   const answer=await typeOracle(path.join(root,config),{at:{file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)},properties:false});
   assert.equal(answer.compiler,'7.0.2');
   observed.push({marker,summary:answer.summary,errorType:answer.errorType});
  }
  fs.mkdirSync('.cc/temp',{recursive:true});
  fs.writeFileSync(`.cc/temp/native-union-subtypes-${config}`,JSON.stringify(observed,null,2));
  assert.deepEqual(observed,[
   {marker:'base',summary:'Base',errorType:false},
   {marker:'reverse',summary:'Base',errorType:false},
   {marker:'discriminated',summary:'Alpha | Beta',errorType:false},
   {marker:'hierarchy',summary:'Parent',errorType:false},
   {marker:'unrelated',summary:'Parent | Sibling',errorType:false},
   {marker:'empty',summary:'Empty',errorType:false},
   {marker:'primitive',summary:'"x" | 1',errorType:false},
   {marker:'constrained',summary:'Alpha | Beta',errorType:false},
   {marker:'aliases',summary:'A | Z',errorType:false},
  ]);
 });
}

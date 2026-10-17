import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-predicate-eligibility/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
for (const profile of ['tsconfig.json','loose.json']) {
 test(`native predicate body eligibility (${profile})`,async()=>{
  const expected={nonBoolean:'"other"',multiple:'"boolean"',bare:'"other"',
   implicit:profile==='tsconfig.json'?'"other"':'"boolean"',unchanged:'"boolean"',
   booleanParameter:profile==='tsconfig.json'?'"other"':'"boolean"',assigned:'"boolean"',
   rest:'"boolean"',inferred:'"predicate"',oneSided:'"boolean"'};
  for (const marker of ['nonBoolean','multiple','bare','implicit','unchanged','booleanParameter','assigned','rest','inferred','oneSided']) {
   const tag=`/*#${marker}*/`;
   const answer=await typeOracle(path.join(root,profile),{at:{file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)},properties:false});
   assert.equal(answer.compiler,'7.0.2');
   assert.equal(answer.errorType,false,`${marker}: ${JSON.stringify(answer)}`);
   assert.equal(answer.summary,expected[marker],`${profile}/${marker}: ${JSON.stringify(answer)}`);
  }
 });
}

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-composite-signatures/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
for (const config of ['tsconfig.json','loose.json','unchecked.json']) {
 test(`native composite calls and index inventories (${config})`,async()=>{
  for (const [marker,summary,errorType] of [
   ['zero','string | number',false],['compatible','1 | 2',false],['partial','1 | 2',false],
   ['disjoint','1 | 2',false],['intersection','1',false],['identical','1',false],
   ['defaults','string',false],['overloaded','1 | 3',false],['this','1 | 2',false],['rest','1 | 2',false],
   ['unionIndex',config==='unchecked.json'?'string | number | undefined':'string | number',false],
   ['intersectionIndex',config==='unchecked.json'?'undefined':'never',false],
   ['unknown','any',true],['badReturn','any',true],['readonlyIndex','number',false],
  ]) {
   const tag=`/*#${marker}*/`;let offset=source.indexOf(tag)+tag.length;
   if (marker==='readonlyIndex') offset=source.indexOf(']',offset);
   const answer=await typeOracle(path.join(root,config),{at:{file:'cases.ts',...position(lineStarts(source),offset)},properties:false});
   assert.equal(answer.compiler,'7.0.2');
   assert.equal(answer.summary,summary,`${marker}: ${JSON.stringify(answer)}`);
   assert.equal(answer.errorType,errorType,`${marker}: ${JSON.stringify(answer)}`);
  }
 });
}

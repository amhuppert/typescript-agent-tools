import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-array-expressions/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected={spreadFirst:'["x", 2, true]',spreadLast:'[true, "x", 2]',spreadMiddle:'[true, "x", 2, false]',spreadMultiple:'["x", 2, true, "x", 2]',spreadEmpty:'[true]', numbers:'number[]',mixed:'(string | number)[]',frozen:'readonly [1, "two"]',hole:'readonly [undefined, 1]',allHole:'readonly [undefined]',nested:'readonly [readonly [1], readonly ["two"]]',spread:'(string | number)[]',tupleSpread:'readonly ["x", 2]',assertion:'string[]',contextTuple:'[1]',contextHoles:'[undefined, string]',contextArray:'("two" | 1)[]',mutableConst:'[1, "two"]',empty:'any[]',parenthesizedEmpty:'never[]'};
for(const config of ['tsconfig.json','loose.json','exact.json']) {
 test(`native array expression contexts (${config})`,async()=>{
  const observed=[];
  for(const marker of Object.keys(expected)) {
   const tag=`/*#${marker}*/`;
   const offset=source.indexOf(tag)+tag.length;
   const answer=await typeOracle(path.join(root,config),{at:{file:'cases.ts',...position(lineStarts(source),offset)},properties:false});
   observed.push({marker,summary:answer.summary,errorType:answer.errorType});
   assert.equal(answer.compiler,'7.0.2');
  }
  fs.mkdirSync('.cc/temp',{recursive:true});
  fs.writeFileSync(`.cc/temp/native-array-expressions-${config}`,JSON.stringify(observed,null,2));
  for(const row of observed) {
   const summary=config==='loose.json' && row.marker==='parenthesizedEmpty'?'any[]':config==='exact.json' && row.marker==='hole'?'readonly [never?, 1?]':config==='exact.json' && row.marker==='allHole'?'readonly [never?]':config==='exact.json' && row.marker==='contextHoles'?'[never?, string?]':config==='loose.json' && row.marker==='hole'?'readonly [any, 1]':config==='loose.json' && row.marker==='allHole'?'readonly [any]':config==='loose.json' && row.marker==='contextHoles'?'[any, string]':expected[row.marker];
   assert.deepEqual([row.summary,row.errorType],[summary,false],row.marker);
  }
 });
}

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/graph-labels/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
test('native same-named aliases keep unqualified root summaries and generic arguments',async()=>{
 const rows=[];
 for(const config of ['tsconfig.json','loose.json','exact.json']) for(const marker of ['forward','reverse','array']){
  const tag=`/*#${marker}*/`;
  const answer=await typeOracle(path.join(root,config),{at:{file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)},properties:marker!=='array'});
  rows.push({config,marker,summary:answer.summary,errorType:answer.errorType,properties:answer.properties?.map(p=>[p.name,p.summary,p.errorType])});
 }
 fs.writeFileSync('.cc/temp/graph-labels-native.json',JSON.stringify(rows,null,2));
 for(const row of rows){
  assert.equal(row.errorType,false);
  assert.equal(row.summary,{forward:'Forward',reverse:'Reverse',array:'Same<string>[]'}[row.marker]);
  if(row.marker!=='array') assert.deepEqual(Object.fromEntries(row.properties.map(([name,summary,error])=>[name,[summary,error]])),{
   first:['Same<string>',false],second:['Same<number>',false],third:['Same<number>',false],repeated:['Same<string>',false],nominalA:['Named',false],nominalB:['Named',false],
  });
 }
});

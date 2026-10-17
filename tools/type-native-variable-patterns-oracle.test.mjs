import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-variable-patterns/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
test('native variable-like declarations preserve pattern/context/default order',async()=>{
 const rows=[];
 for(const config of ['tsconfig','loose','exact','unchecked']){
  for(const match of source.matchAll(/\/\*#([^*]+)\*\//g)){
   const answer=await typeOracle(path.join(root,config+'.json'),{at:{file:'cases.ts',...position(lineStarts(source),match.index+match[0].length)},properties:false});
   assert.equal(answer.compiler,'7.0.2');
   rows.push({config,name:match[1],summary:answer.summary,errorType:answer.errorType});
  }
 }
 fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/types-variable-patterns-native.json',JSON.stringify(rows,null,2));
 assert.deepEqual(rows,JSON.parse(fs.readFileSync(path.join(root,'expected.json'),'utf8')));
});

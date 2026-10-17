import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-indexed-access/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const summaries={plain:'number',union:'string | number',absent:'any',consumed:'Missing',unused:'number',indexed:'number',numbered:'string',neverKey:'never',anyObject:'any',neverObject:'never',unknownObject:'any',invalidKey:'any',indexedUnion:'string | number',indexedIntersection:'never',readonlyIndex:'number',instantiated:'42',generic:'T["p"]',expression:'number',genericKey:'Box[K]',unionGeneric:'T["p" | "q"]',constrainedIndex:'Box[K]'};
Object.assign(summaries,{negativeMixed:'number | bigint',tildeMixed:'number | bigint',negativeBig:'bigint',tildeBig:'bigint',negativeNumber:'number',tildeNumber:'number',negativeGeneric:'number',tildeGeneric:'number'});
const errors=new Set(['absent','consumed','unknownObject','invalidKey']);
for (const config of ['tsconfig.json','loose.json','exact.json','unchecked.json']) {
 test(`native shared indexed access (${config})`,async()=>{
  const evidence=[];
  const mismatches=[];
  for (const marker of ['plain','optional',...Object.keys(summaries).filter(x=>x!=='plain')]) {
   const tag=`/*#${marker}*/`;let offset=source.indexOf(tag);
   offset=offset<0?source.indexOf(`const ${marker}:`)+6:offset+tag.length;
   if (['expression','genericKey'].includes(marker)) offset=source.indexOf(']',offset);
   assert.ok(offset>=6,marker);
   const answer=await typeOracle(path.join(root,config),{at:{file:'cases.ts',...position(lineStarts(source),offset)},properties:false});
   evidence.push({marker,summary:answer.summary,errorType:answer.errorType,kind:answer.kind});
   fs.mkdirSync('.cc/temp',{recursive:true});
   fs.writeFileSync(`.cc/temp/native-indexed-access-${config}`,JSON.stringify(evidence,null,2));
   assert.equal(answer.compiler,'7.0.2');
   const summary=marker==='optional'?(config==='loose.json'?'boolean':'boolean | undefined'):summaries[marker];
   if (answer.summary!==summary||answer.errorType!==errors.has(marker)) mismatches.push({marker,expected:summary,actual:answer.summary,errorType:answer.errorType});
  }
  assert.deepEqual(mismatches,[]);
 });
}

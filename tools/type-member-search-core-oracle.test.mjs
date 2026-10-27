import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';

const root=fileURLToPath(new URL('../fixtures/types/member-search-core/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const markers=['declaration','first','second','other','otherUse','inlineLeft','inlineRight','inlineUse','interfaceLeft','interfaceRight','interfaceUse','escapedDeclaration','escapedUse','literalUse','numericDeclaration','numericUse','numericString'];
function at(marker,length,offset=0) {
 const tag=`/*#${marker}*/`,start=source.indexOf(tag)+tag.length+offset;
 return {file:'cases.ts',start:position(lineStarts(source),start),end:position(lineStarts(source),start+length)};
}
const origin=(marker,length,kind='PropertySignature')=>({kind,location:at(marker,length)});
const group=(origins,occurrences)=>({origins,occurrences});
test('native directed member search and decoded candidate normalization',async()=>{
 const rows=[];
 for(const config of ['tsconfig.json','loose.json','exact.json']) for(const marker of markers){
  const tag=`/*#${marker}*/`;
  const answer=await compilerHookOracle(path.join(root,config),{operation:'member',at:{file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)}});
  rows.push({config,marker,answer});
 }
 fs.writeFileSync(scratchPath('member-search-core-native.json'),JSON.stringify(rows,null,2));
 assert.equal(rows.length,51);
 const generic=[group([origin('declaration',5)],['declaration','first','second'].map(x=>at(x,5)))];
 const other=[group([origin('other',5)],['other','otherUse'].map(x=>at(x,5)))];
 const left=origin('inlineLeft',4),right=origin('inlineRight',4);
 const inline=[group([left],[at('inlineLeft',4)]),group([left,right],[at('inlineUse',4)]),group([right],[at('inlineRight',4)])];
 const interfaceLeft=group([origin('interfaceLeft',4)],[at('interfaceLeft',4)]);
 const interfaceRight=group([origin('interfaceRight',4)],[at('interfaceRight',4)]);
 const interfaceUse=group([origin('interfaceLeft',4),origin('interfaceRight',4)],[at('interfaceUse',4)]);
 for(const {config,marker,answer} of rows){
  assert.deepEqual(answer.demandFailures,[],marker);
  if(['declaration','first','second'].includes(marker)) assert.deepEqual(answer.groups,generic,marker);
  else if(['other','otherUse'].includes(marker)) assert.deepEqual(answer.groups,other,marker);
  else if(marker.startsWith('inline')) assert.deepEqual(answer.groups,inline,marker);
  else if(marker.startsWith('interface')) assert.deepEqual(answer.groups,marker==='interfaceLeft'?[interfaceLeft,interfaceUse]:marker==='interfaceRight'?[interfaceUse,interfaceRight]:[interfaceLeft,interfaceUse,interfaceRight],marker);
  else {
   const escaped=marker.startsWith('escaped')||marker==='literalUse';
   const declaration=escaped?origin('escapedDeclaration',8,'MethodSignature'):origin('numericDeclaration',4);
   const uses=escaped?[at('escapedUse',3),at('literalUse',5)]:[at('numericUse',2),at('numericString',4)];
   assert.deepEqual(answer.groups,[group([declaration],uses)],`${marker}: raw spelling search omits the alternate-spelled declaration`);
   // The hook records the native getRelatedSymbol result for each exact
   // decoded candidate before raw text search. This is the explicit product
   // normalization; raw LS groups above remain preserved independently.
   assert.deepEqual(answer.candidateObservations.map(row=>row.relatedOrigins),[[declaration],[declaration],[declaration]],marker);
   assert.deepEqual(answer.candidateObservations.map(row=>row.location),[declaration.location,...uses],marker);
   const nativeUses=escaped?[at('escapedUse',3),at('literalUse',3,1)]:[at('numericUse',2),at('numericString',2,1)];
   // Raw name nodes above remain the decoded-candidate inventory and full
   // declaration origins. Native reported occurrences exclude nonempty quotes.
   assert.deepEqual(answer.nativeRangeGroups,[group([declaration],nativeUses)],marker);
   assert.deepEqual(answer.candidateObservations.map(row=>row.nativeRange),[declaration.location,...nativeUses],marker);
   assert.deepEqual(answer.candidateObservations.map(row=>row.rawTextMatch),[false,true,true],marker);
   assert.ok(answer.candidateObservations.every(row=>row.validRaw),marker);
  }
  if(['declaration','first','second','other','otherUse'].includes(marker)){
   const noSymbols=answer.candidateObservations.filter(row=>[26,28].includes(row.location.start.line));
   assert.equal(noSymbols.length,2);
   assert.ok(noSymbols.every(row=>row.symbolFlags===0&&row.origins.length===0));
   assert.ok(answer.diagnosticCodes.includes(config==='loose.json'?2339:18046),'unknown access takes native diagnostic recovery');
  }
 }
});

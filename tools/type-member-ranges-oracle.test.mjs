import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';
const root=fileURLToPath(new URL('../fixtures/types/member-reference-ranges/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected={plain:[[2,14,19],[9,8,13],[10,8,13],[11,7,12]],escaped:[[12,8,15],[13,8,15]],astral:[[4,15,17],[14,8,10],[15,8,10]],empty:[],number:[[6,14,16],[18,7,9]]};
const normalized={...expected,escaped:[[3,16,28],[12,8,15],[13,8,15]],empty:[[5,13,15],[16,7,9],[17,7,9]],number:[[6,14,16],[18,7,9],[19,7,11]]};
test('native final reference ranges preserve raw nodes and strip only nonempty literal delimiters',async()=>{
 const rows=[];
 for(const marker of Object.keys(expected)){
  const tag=`/*#${marker}*/`, at={file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)};
  const answer=await compilerHookOracle(path.join(root,'tsconfig.json'),{operation:'member',at});
  rows.push({marker,answer});
  fs.writeFileSync(scratchPath('member-ranges-native.json'),JSON.stringify(rows,null,2));
 }
 for(const {marker,answer} of rows){
  assert.deepEqual(answer.candidateObservations.map(v=>[v.nativeRange.start.line,v.nativeRange.start.column,v.nativeRange.end.column]),normalized[marker],`${marker} decoded inventory`);
  assert.equal(answer.nativeRangeGroups.length,marker==='empty'?0:1,marker);
  if(marker==='empty') continue;
  assert.deepEqual(answer.nativeRangeGroups[0].occurrences.map(v=>[v.start.line,v.start.column,v.end.column]),expected[marker],marker);
  assert.deepEqual(answer.nativeRangeGroups[0].origins,answer.groups[0].origins,'definition origins keep native declaration-name nodes');
  assert.equal(answer.groups[0].occurrences.length,answer.nativeRangeGroups[0].occurrences.length);
 }
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';

const root=fileURLToPath(new URL('../fixtures/types/member-search-inventory/',import.meta.url));
const source=fs.readFileSync(path.join(root,'scope.ts'),'utf8');
test('native cold private scope searches retain their actual byte-offset behavior',async()=>{
 const rows=[];
 for(const config of ['tsconfig.json','loose.json','exact.json']){
  for(const marker of ['privateDeclaration','otherPrivateDeclaration']){
   const tag=`/*#${marker}*/`;
   const at={file:'scope.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)};
   const answer=await compilerHookOracle(path.join(root,config),{operation:'member',history:'cold',at});
   rows.push({config,marker,answer});
  }
 }
 fs.writeFileSync(scratchPath('member-scope-native.json'),JSON.stringify(rows,null,2));
 assert.equal(rows.length,6);
 for(const {config,marker,answer} of rows){
  assert.equal(answer.history,'cold');
  assert.deepEqual(answer.candidateDemands,[],'the normalized candidate callback is disabled');
  const expected=marker==='privateDeclaration'?[]:[config==='loose.json'?2339:18046];
  assert.deepEqual(answer.diagnosticCodes.filter(code=>code!==2318),expected,`${config}/${marker}`);
  assert.equal(answer.groups.length,1);
  assert.equal(answer.groups[0].occurrences.length,2);
  assert.deepEqual(answer.groups[0].occurrences.map(location=>location.start.line),marker==='privateDeclaration'?[2,3]:[8,9]);
 }
});

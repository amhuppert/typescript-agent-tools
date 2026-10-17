import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {test} from 'node:test';
import {compilerHookOracle,typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
import {subtypeRemovalSource} from '../fixtures/types/compiler-limits/generate.mjs';
const globals=fs.readFileSync(new URL('../fixtures/types/native-array-expressions/globals.d.ts',import.meta.url),'utf8');
async function probe(t,source,strict,hook) {
 const directory=fs.mkdtempSync(path.join(os.tmpdir(),'tsparse-union-limit-'));
 t.after(()=>fs.rmSync(directory,{recursive:true,force:true}));
 fs.writeFileSync(path.join(directory,'cases.ts'),source);
 fs.writeFileSync(path.join(directory,'globals.d.ts'),globals);
 fs.writeFileSync(path.join(directory,'tsconfig.json'),JSON.stringify({compilerOptions:{strict,noLib:true},files:['cases.ts','globals.d.ts']}));
 const at={file:'cases.ts',...position(lineStarts(source),source.indexOf('/*#query*/')+10)};
 return hook?compilerHookOracle(path.join(directory,'tsconfig.json'),{at,inspectArrayElement:true}):typeOracle(path.join(directory,'tsconfig.json'),{at,properties:false});
}
for(const strict of [true,false]) {
 test(`subtype work estimate preserves exact natural boundaries (strict=${strict})`,async t=>{
  for(const [removals,processed,estimate,constituents,invalid] of [[43,126,991250,1207,false],[42,125,1000000,1208,false],[41,124,1007500,1,true]]) {
   const answer=await probe(t,subtypeRemovalSource(1250,removals),strict,true);
   assert.equal(answer.errorType,false);
   assert.deepEqual(answer.arrayElement,{errorType:invalid,constituents});
   assert.deepEqual(answer.beforeSummary.observations.subtypeEstimate,{count:100000,length:1250,processed,estimate});
   assert.deepEqual(answer.beforeSummary.demandFailures,invalid?['subtype_complexity_limit']:[]);
   assert.deepEqual(answer.beforeSummary.diagnosticCodes,invalid?[2590]:[]);
  }
 });
 test(`flat shallow array does not spend subtype pairs (strict=${strict})`,async t=>{
  const answer=await probe(t,`declare const value: number;\nconst /*#query*/query=[${Array(10001).fill('value').join(',')}];\n`,strict,false);
  assert.deepEqual([answer.summary,answer.errorType],['number[]',false]);
 });
}

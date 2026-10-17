import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle,typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-reporting-relations/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected=JSON.parse(fs.readFileSync(path.join(root,'expected.json'),'utf8'));
const markers=[...source.matchAll(/\/\*#([^*]+)\*\//g)].map(match=>({name:match[1],offset:match.index+match[0].length}));
for(const config of ['tsconfig','exact','loose']) test(`native reporting relation dependencies (${config})`,async t=>{
 const rows=[];
 for(const {name,offset} of markers){
  const at={file:'cases.ts',...position(lineStarts(source),offset)};
  const answer=await typeOracle(path.join(root,config+'.json'),{at,properties:false});
  const hook=await compilerHookOracle(path.join(root,config+'.json'),{at});
  assert.equal(answer.compiler,'7.0.2');assert.equal(answer.summary,hook.summary);
  rows.push({name,summary:answer.summary,errorType:answer.errorType,codes:hook.beforeSummary.diagnosticCodes,observations:hook.beforeSummary.observations});
 }
 fs.mkdirSync('.cc/temp',{recursive:true});
 fs.writeFileSync(`.cc/temp/native-reporting-relations-${config}.json`,JSON.stringify(rows,null,2));
 for(const row of rows){
  const pinned=expected.find(item=>item.name===row.name);
  assert.equal(row.summary,pinned.summary,row.name);
  assert.equal(row.errorType,pinned.errorType,row.name);
  assert.deepEqual(row.codes,pinned.codes,row.name);
 }
 const returns=(row,fragment)=>Object.entries(row.observations)
  .filter(([key])=>key.startsWith('signatureReturn:'))
  .map(([,value])=>value).filter(value=>value.pos<=source.indexOf(fragment)&&value.end>=source.indexOf(fragment)+fragment.length);
 for(const [prefix,fragment] of [['weakCall','MissingCallReturn'],['weakConstruct','MissingConstructReturn']]){
  assert.deepEqual(returns(rows.find(row=>row.name===prefix+'Silent'),fragment),[]);
  assert.deepEqual(returns(rows.find(row=>row.name===prefix+'Report'),fragment).map(value=>value.errorType),[true]);
 }
 assert.deepEqual(returns(rows.find(row=>row.name==='weakBothReport'),'MissingSkippedConstruct'),[]);
 assert.deepEqual(returns(rows.find(row=>row.name==='weakSecondSilent'),'MissingSecondConstruct'),[]);
 assert.deepEqual(returns(rows.find(row=>row.name==='weakSecondReport'),'MissingSecondConstruct').map(value=>value.errorType),[true]);
 t.diagnostic(`Captured ${rows.length} final roots and pinned reporting-only dependency schedules`);
});

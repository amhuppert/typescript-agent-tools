import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-unknown-signature/',import.meta.url));
for(const [config,file] of [['tsconfig','warm'],['exact','warm'],['loose','warm'],['cold','cold'],['reordered','reordered']])
 test(`native unknown signature shared checker (${config})`,async t=>{
  const source=fs.readFileSync(path.join(root,file+'.ts'),'utf8');
  const at=name=>({file:file+'.ts',...position(lineStarts(source),source.indexOf(`/*#${name}*/`)+name.length+5)});
  const member=await compilerHookOracle(path.join(root,config+'.json'),{operation:'member',at:at('seed')});
  const first=await compilerHookOracle(path.join(root,config+'.json'),{at:at('firstCall')});
  fs.mkdirSync('.cc/temp',{recursive:true});
  fs.writeFileSync(`.cc/temp/native-unknown-signature-${config}.json`,JSON.stringify({member,first},null,2));
  assert.equal(member.schedule.checker,'one');
  assert.equal(member.schedule.relationshipBeforeCandidates,true);
  assert.equal(member.schedule.searchSetCount,1);
  assert.equal(member.schedule.candidates,4);
  const location=name=>({file:file+'.ts',start:at(name),end:{...at(name),column:at(name).column+5}});
  const exactLocation=name=>{
   const {file,start,end}=location(name);
   return {file,start:{line:start.line,column:start.column},end:{line:end.line,column:end.column}};
  };
  const origin={kind:'PropertySignature',location:exactLocation('seed')};
  assert.deepEqual(member.groups,[{origins:[origin],occurrences:[exactLocation('seed'),exactLocation('warm')]}]);
  const ordered=file==='reordered'?['seed','first','warm','second']:['seed','warm','first','second'];
  assert.deepEqual(member.candidateObservations.map(row=>({location:row.location,origins:row.origins})),
   ordered.map(name=>({location:exactLocation(name),origins:name==='seed'||name==='warm'?[origin]:[]})));
  const constructors=Object.entries(member.observations).filter(([key])=>key.startsWith('constructorResolution:'));
  const count=file==='cold'?2:3;
  assert.equal(constructors.length,count);
  for(const [,observation] of constructors) assert.deepEqual(observation,{anySignature:false,silentNeverSignature:false,unknownSignature:true});
  assert.deepEqual(member.diagnosticCodes,Array(count).fill(2351));
  assert.deepEqual(member.demandFailures,[]);
  assert.equal(member.prelude.length,file==='cold'?0:1);
  if(file!=='cold') {
   const begin=source.indexOf('new NotConstructor(healthy.');
   const end=source.indexOf(');',begin)+1;
   assert.deepEqual(member.prelude,[{kind:'NewExpression',location:{file:file+'.ts',start:position(lineStarts(source),begin),end:position(lineStarts(source),end)}}]);
  }
  assert.equal(first.summary,'any');
  assert.equal(first.errorType,true);
  assert.deepEqual(first.beforeSummary.diagnosticCodes,[2351]);
  assert.deepEqual(first.beforeSummary.demandFailures,[]);
  t.diagnostic('Asserted actual singleton equality at constructors, same-checker member schedule, and final error return');
 });

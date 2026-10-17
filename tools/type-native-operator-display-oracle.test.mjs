import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle,typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-operator-display/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const markers=[...source.matchAll(/\/\*#([^*]+)\*\//g)].map(match=>({name:match[1],offset:match.index+match[0].length}));
const failures=new Set(['left','right','notEqual','looseEqual','looseNotEqual','initializer','awaited','awaitedReverse']);
test('native equality diagnostics consume exact operand display dependencies',async t=>{
 const rows=[];
 for(const config of ['tsconfig','loose','exact']) for(const {name,offset} of markers){
  const answer=await compilerHookOracle(path.join(root,config+'.json'),{at:{file:'cases.ts',...position(lineStarts(source),offset)}});
  rows.push({config,name,answer});
 }
 fs.mkdirSync('.cc/temp',{recursive:true});
 fs.writeFileSync('.cc/temp/native-operator-display-capture.json',JSON.stringify(rows,null,2));
 for(const {config,name,answer} of rows){
  const label=`${config}/${name}`;
  const codes=failures.has(name)?[2304,2367]:['literals','healthy'].includes(name)?[2367]:[];
  assert.equal(answer.compiler,'7.0.2',label);
  assert.equal(answer.summary,name==='initializer'?'boolean':'"checked"',label);
  assert.equal(answer.errorType,false,label);
  assert.deepEqual(answer.beforeSummary.diagnosticCodes,codes,`${label}/before summary`);
  assert.deepEqual(answer.diagnosticCodes,codes,label);
  assert.deepEqual(answer.beforeSummary.demandFailures,[],label);
  assert.deepEqual(answer.demandFailures,[],label);
  assert.deepEqual(answer.beforeSummary.observations.nonMissingSymbolErrorType,
   failures.has(name)?{instantiated:false,name:'hidden'}:undefined,`${label}/lazy annotation`);
 }
 const diagnostics=await typeOracle(path.join(root,'tsconfig.json'),{at:{file:'cases.ts',...position(lineStarts(source),markers.find(marker=>marker.name==='literals').offset)},diagnostics:true});
 fs.writeFileSync('.cc/temp/native-operator-display-diagnostics.json',JSON.stringify(diagnostics,null,2));
 const message=name=>diagnostics.diagnostics.find(row=>row.code===2367&&row.location.start.line===position(lineStarts(source),markers.find(marker=>marker.name===name).offset).line)?.message;
 assert.equal(message('left'),"This comparison appears to be unintentional because the types '{ tag: 0; hidden?: any; }' and 'number' have no overlap.");
 assert.equal(message('right'),"This comparison appears to be unintentional because the types 'number' and '{ tag: 0; hidden?: any; }' have no overlap.");
 assert.equal(message('literals'),"This comparison appears to be unintentional because the types '1' and '2' have no overlap.");
 assert.equal(message('awaited'),"This comparison appears to be unintentional because the types 'Promise<{ tag: 0; hidden?: any; }>' and '{ tag: 0; }' have no overlap.");
 t.diagnostic('39 fresh checker queries pin display-only failures, quick-call skipping, operand order, and original literals versus unrelated bases');
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-deprecated-recovery/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const markers=[...source.matchAll(/\/\*#([^*]+)\*\//g)].map(match=>({name:match[1],offset:match.index+match[0].length}));
for(const config of ['tsconfig','exact','loose']) test(`native JSDoc grammar recovery (${config})`,async t=>{
 const rows=[];
 for(const {name,offset} of markers){
  const hook=await compilerHookOracle(path.join(root,config+'.json'),{at:{file:'cases.ts',...position(lineStarts(source),offset)}});
  rows.push({name,summary:hook.summary,errorType:hook.errorType,before:hook.beforeSummary});
  assert.equal(hook.summary,JSON.stringify(name),`${config}/${name}: final native type`);
  assert.equal(hook.errorType,false,`${config}/${name}: recovered comment leaves root healthy`);
  assert.deepEqual(hook.beforeSummary.diagnosticCodes,[2304],`${config}/${name}: hidden annotation consumed before summary`);
  assert.deepEqual(hook.beforeSummary.observations.nonMissingSymbolErrorType,{instantiated:false,name:'bad'},`${config}/${name}: actual omitted property read`);
 }
 fs.mkdirSync('.cc/temp',{recursive:true});
 fs.writeFileSync(`.cc/temp/native-deprecated-recovery-${config}.json`,JSON.stringify(rows,null,2));
 assert.equal(rows.length,markers.length);
 t.diagnostic(`Captured ${rows.length} final roots and pre-summary dependency observations`);
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-deprecated-context/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const markers=[...source.matchAll(/\/\*#([^*]+)\*\//g)].map(match=>({name:match[1],offset:match.index+match[0].length}));
// Native eager parsing keeps the enclosing Await/Yield context; lazy parsing
// starts fresh. Captured in e909df30 before these assertions were installed.
const consumes=new Set(['ordinaryAwait','ordinaryYield']);
for(const config of ['tsconfig','exact','loose']) test(`native eager JSDoc context (${config})`,async t=>{
 const rows=[];
 for(const {name,offset} of markers){
  const hook=await compilerHookOracle(path.join(root,config+'.json'),{at:{file:'cases.ts',...position(lineStarts(source),offset)}});
  rows.push({name,summary:hook.summary,errorType:hook.errorType,before:hook.beforeSummary});
  assert.equal(hook.summary,JSON.stringify(name),`${config}/${name}: final native type`);
  assert.equal(hook.errorType,false,`${config}/${name}: final root remains healthy`);
  assert.deepEqual(hook.beforeSummary.diagnosticCodes,consumes.has(name)?[2304]:[],`${config}/${name}: context controls deprecated display`);
  assert.deepEqual(hook.beforeSummary.observations.nonMissingSymbolErrorType,consumes.has(name)?{instantiated:false,name:'bad'}:undefined,`${config}/${name}: actual omitted annotation acquisition`);
 }
 fs.mkdirSync('.cc/temp',{recursive:true});
 fs.writeFileSync(`.cc/temp/native-deprecated-context-${config}.json`,JSON.stringify(rows,null,2));
 assert.equal(rows.length,markers.length);
 t.diagnostic(`Captured ${rows.length} final roots and pre-summary dependency observations`);
});

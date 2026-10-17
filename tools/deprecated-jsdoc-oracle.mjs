import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-deprecated-jsdoc/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const markers=[...source.matchAll(/\/\*#([^*]+)\*\//g)].map(match=>({name:match[1],offset:match.index+match[0].length}));
// Scanner eligibility is distinct from lazy tag confirmation. These expected
// sets were first captured from the pinned compiler before asserting them.
const consumes=new Set(['space','tab','cr','lf','crlf','brace','asterisk','bare','eagerNbsp','eagerEscaped','param','paramDefault','paramQuoted','paramNested','template','typedef','typedefType','callback','overload','implements','augments','import','importDefault','importNamespace','satisfies','throws','this','type','unclosedType','arrow','fn','method','qualified']);
// Separate registered files keep each profile inside the harness time budget;
// all expected outcomes and compiler calls stay in this one owner.
export function deprecatedJsdocProfile(config) { test(`native JSDoc deprecation grammar (${config})`,async t=>{
 const rows=[];
 for(const {name,offset} of markers){
  const hook=await compilerHookOracle(path.join(root,config+'.json'),{at:{file:'cases.ts',...position(lineStarts(source),offset)}});
  rows.push({name,summary:hook.summary,errorType:hook.errorType,before:hook.beforeSummary});
  assert.equal(hook.summary,JSON.stringify(name),`${config}/${name}: final native type`);
  assert.equal(hook.errorType,false,`${config}/${name}: final native type is healthy`);
  assert.deepEqual(hook.beforeSummary.diagnosticCodes,consumes.has(name)?[2304]:[],`${config}/${name}: pre-summary dependency diagnostics`);
  if(consumes.has(name)&&name!=='qualified'&&name!=='method') assert.equal(hook.beforeSummary.observations.nonMissingSymbolErrorType?.name,'bad',`${config}/${name}: omitted property type was consumed`);
  if(name==='method') assert.equal(hook.beforeSummary.observations.nonMissingSymbolErrorType,undefined,`${config}/${name}: method checking consumes the annotation before property display`);
  if(name==='qualified') assert.deepEqual(hook.beforeSummary.observations.typeParameterDefaultError,{hasSymbol:true,parameter:'T'},`${config}/${name}: qualified class default was consumed`);
  if(name==='qualifiedNormal') assert.equal(hook.beforeSummary.observations.typeParameterDefaultError,undefined,`${config}/${name}: ordinary call leaves class default lazy`);
 }
 fs.mkdirSync('.cc/temp',{recursive:true});
 fs.writeFileSync(`.cc/temp/native-deprecated-jsdoc-${config}.json`,JSON.stringify(rows,null,2));
 assert.equal(rows.length,markers.length);
 t.diagnostic(`Captured ${rows.length} final roots and pre-summary dependency observations`);
}); }

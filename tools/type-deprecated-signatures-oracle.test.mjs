import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-deprecated-signatures/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const markers=[...source.matchAll(/\/\*#([^*]+)\*\//g)].map(match=>({name:match[1],offset:match.index+match[0].length}));
const summaries={
 normal:'"normal"', deprecated:'"deprecated"',
 quickNormal:'{ value: "normal"; }', quickDeprecated:'{ value: "deprecated"; }',
 normalDefault:'"normalDefault"', deprecatedDefault:'"deprecatedDefault"', healthy:'"healthy"',
 notDeprecated:'"notDeprecated"', normalClass:'"normalClass"', deprecatedClass:'"deprecatedClass"',
 quickDeprecatedClass:'DeprecatedClass', normalNew:'any', deprecatedNew:'any',
 overloadNormal:'"number"', overloadDeprecated:'"string"', inlineCode:'"inlineCode"',
 fencedCode:'"fencedCode"', hyphenTag:'"hyphenTag"', emailText:'"emailText"',
 nbspTag:'"nbspTag"', linkText:'"linkText"', secondTag:'"secondTag"', tag:'"tag"',
};
const missingProperty=new Set(['deprecated','deprecatedClass','overloadDeprecated','secondTag','tag']);
for(const config of ['tsconfig','exact','loose']) test(`native deprecated signature dependencies (${config})`,async t=>{
 const rows=[];
 for(const {name,offset} of markers){
  const hook=await compilerHookOracle(path.join(root,config+'.json'),{at:{file:'cases.ts',...position(lineStarts(source),offset)}});
  rows.push({name,summary:hook.summary,errorType:hook.errorType,before:hook.beforeSummary});
 }
 fs.mkdirSync('.cc/temp',{recursive:true});
 fs.writeFileSync(`.cc/temp/native-deprecated-signatures-${config}.json`,JSON.stringify(rows,null,2));
 assert.equal(rows.length,23);
 for(const row of rows){
  const label=`${config}/${row.name}`;
  assert.equal(row.summary,summaries[row.name],label);
  assert.equal(row.errorType,false,label);
  const codes=row.name==='normalNew' ? (config==='loose'?[2304,2350]:[7009])
   : row.name==='deprecatedNew' ? (config==='loose'?[2304,2350]:[2304,7009])
   : row.name==='deprecatedClass' ? [2552]
   : missingProperty.has(row.name)||row.name==='deprecatedDefault' ? [2304] : [];
  assert.deepEqual(row.before.diagnosticCodes,codes,label);
  assert.deepEqual(row.before.demandFailures,[],label);
  assert.deepEqual(row.before.observations.nonMissingSymbolErrorType,
   missingProperty.has(row.name)?{instantiated:false,name:'bad'}:undefined,label);
  assert.deepEqual(row.before.observations.typeParameterDefaultError,
   row.name==='deprecatedDefault'?{hasSymbol:true,parameter:'T'}:undefined,label);
  const badReturn=Object.entries(row.before.observations).some(([key,value])=>key.startsWith('signatureReturn:')&&value.errorType);
  assert.equal(badReturn,row.name==='deprecatedNew'||row.name==='normalNew'&&config==='loose',label);
 }
 t.diagnostic(`Asserted ${rows.length} final roots and pre-summary dependency observations`);
});

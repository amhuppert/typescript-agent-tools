import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle,typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-reference-contexts/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
test('native value references inside types keep expression and query contexts distinct',async t=>{
 const rows=[];
 for(const match of source.matchAll(/\/\*#([^*]+)\*\//g)){
  const name=match[1];
  const at={file:'cases.ts',...position(lineStarts(source),match.index+match[0].length)};
  const answer=await typeOracle(path.join(root,'tsconfig.json'),{at});
  const hook=await compilerHookOracle(path.join(root,'tsconfig.json'),{operation:'type',at});
  assert.equal(answer.compiler,'7.0.2');assert.equal(answer.summary,hook.summary,name);
  rows.push({name,summary:answer.summary,error:answer.errorType,codes:hook.beforeSummary.diagnosticCodes,failures:hook.beforeSummary.demandFailures});
 }
 fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/types-reference-contexts-native.json',JSON.stringify(rows,null,2));
 t.diagnostic(JSON.stringify(rows));
 for(const row of rows){
  const missing=row.name.startsWith('missing');
  assert.equal(row.summary,missing?'any':row.name.startsWith('imported')?'"imported"':'"stable"',row.name);
  assert.equal(row.error,missing,row.name);
  assert.deepEqual(row.codes,missing?[2304]:[],row.name);
  assert.deepEqual(row.failures,[],row.name);
 }
});

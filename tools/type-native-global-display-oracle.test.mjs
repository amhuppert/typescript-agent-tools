import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-global-display/',import.meta.url));
test('native generic display consumes iterator declaration before its arity gate',async t=>{
 const rows=[];
 for(const config of ['tsconfig','one','three','this','plain']){
  const file=config==='tsconfig'?'cases.ts':config+'.ts';
  const source=fs.readFileSync(path.join(root,file),'utf8');
  for(const match of source.matchAll(/\/\*#([^*]+)\*\//g)){
   const at={file,...position(lineStarts(source),match.index+match[0].length)};
   const answer=await compilerHookOracle(path.join(root,config+'.json'),{operation:'type',at});
   assert.equal(answer.compiler,'7.0.2');
   const name=match[1];
   const row={config,name,summary:answer.summary,errorType:answer.errorType,before:answer.beforeSummary.diagnosticCodes,after:answer.diagnosticCodes,failures:answer.demandFailures};
   rows.push(row);
   assert.equal(row.summary,{ordinary:'Box<number>',array:'number[]',nongeneric:'{ value: number; }'}[name],`${config}/${name}`);
   assert.equal(row.errorType,false);
   assert.deepEqual(row.before,[],`${config}/${name}/cold`);
   assert.deepEqual(row.after,config==='tsconfig'&&name==='ordinary'?[2307]:[],`${config}/${name}/display`);
   assert.deepEqual(row.failures,[]);
  }
 }
 fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/types-global-display-native.json',JSON.stringify(rows,null,2));
 t.diagnostic(JSON.stringify(rows));
});

test('admitted iterator-global arity controls retain ordinary generic summaries', async () => {
 const rows=[];
 for(const config of ['tsconfig','one','three','this','plain']){
  const file=config==='tsconfig'?'cases.ts':config+'.ts';
  const source=fs.readFileSync(path.join(root,'admitted',file),'utf8');
  for(const match of source.matchAll(/\/\*#([^*]+)\*\//g)){
   const at={file,...position(lineStarts(source),match.index+match[0].length)};
   const answer=await compilerHookOracle(path.join(root,'admitted',config+'.json'),{operation:'type',at});
   assert.equal(answer.compiler,'7.0.2');
   assert.equal(answer.summary,{ordinary:'Box<number>',array:'number[]',nongeneric:'{ value: number; }'}[match[1]]);
   assert.equal(answer.errorType,false);
   assert.deepEqual(answer.diagnosticCodes,[]);
   assert.deepEqual(answer.demandFailures,[]);
   rows.push({config,name:match[1],summary:answer.summary,errorType:answer.errorType});
  }
 }
 fs.mkdirSync('.cc/temp',{recursive:true});
 fs.writeFileSync('.cc/temp/types-global-display-admitted-native.json',JSON.stringify(rows,null,2)+'\n');
 assert.equal(rows.length,15);
});

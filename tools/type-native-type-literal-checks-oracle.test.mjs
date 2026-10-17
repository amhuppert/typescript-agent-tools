import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle,typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-type-literal-checks/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected=JSON.parse(fs.readFileSync(path.join(root,'expected-diagnostics.json'),'utf8'));
const markers=[...source.matchAll(/\/\*#([^*]+)\*\//g)].map(match=>({name:match[1],offset:match.index+match[0].length}));
test('native TypeLiteral source checks preserve member, index and duplicate demand order',async t=>{
 const rows=[];
 for(const config of ['tsconfig.json','loose.json','exact.json']) for(const {name,offset} of markers){
  const at={file:'cases.ts',...position(lineStarts(source),offset)};
  const answer=await typeOracle(path.join(root,config),{at,properties:false});
  const hook=await compilerHookOracle(path.join(root,config),{operation:'type',at});
  assert.equal(answer.compiler,'7.0.2');assert.equal(answer.summary,hook.summary,`${config}/${name}/hook`);
  rows.push({config,name,summary:answer.summary,error:answer.errorType,codes:hook.beforeSummary.diagnosticCodes,failures:hook.beforeSummary.demandFailures});
 }
 fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/types-type-literal-checks-native.json',JSON.stringify(rows,null,2));
 t.diagnostic(JSON.stringify(rows));
 for(const row of rows){assert.deepEqual(row.codes,expected[`${row.config}/${row.name}`],`${row.config}/${row.name}/diagnostic dependencies`);assert.equal(row.summary,row.name==='quick'?'{ value: number; }':'number',`${row.config}/${row.name}`);assert.equal(row.error,false,`${row.config}/${row.name}`);assert.deepEqual(row.failures,[]);}
});

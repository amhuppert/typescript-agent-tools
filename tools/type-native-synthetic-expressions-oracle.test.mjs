import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle,typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-synthetic-expressions/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected={ordinary:'string',spread:'number',rawRest:'[flag: boolean, ...values: number[]]',generic:'<T>(value: T) => T',consumed:'Broken',healthyAfter:'number',fixedContext:'number',optionalContext:'number',arrayContext:'number',brokenContext:'MissingContextualIndex'};
test('synthetic checking distinguishes numeric spread values, raw rest data and generic post-check dependencies',async t=>{
 const rows=[];
 for(const config of ['tsconfig.json','loose.json','exact.json']) for(const [marker,summary] of Object.entries(expected)) {
  const tag=`/*#${marker}*/`;
  const at={file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)};
  const answer=await typeOracle(path.join(root,config),{at});
  const hook=await compilerHookOracle(path.join(root,config),{operation:'type',at});
  assert.equal(answer.summary,hook.summary);
  rows.push({config,marker,summary:answer.summary,error:answer.errorType,codes:hook.beforeSummary.diagnosticCodes,failures:hook.beforeSummary.demandFailures});
 }
 fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/types-synthetic-expressions-observations.json',JSON.stringify(rows,null,2));
 t.diagnostic(JSON.stringify(rows));
 for(const row of rows){assert.equal(row.summary,expected[row.marker],`${row.config}/${row.marker}`);assert.equal(row.error,row.marker==='brokenContext');assert.deepEqual(row.failures,[]);assert.deepEqual(row.codes,['consumed','brokenContext'].includes(row.marker)?[2304]:[],`${row.config}/${row.marker}/dependencies`);}
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-quick-expressions/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected={literal:'1',boolean:'true',parenthesized:'1',assertion:'number',parenthesizedAssertion:'string',call:'number',awaitLiteral:'1',awaitAssertion:'string',awaitCall:'number',awaitPromise:'number',awaitScalar:'number',awaitBad:'any',constFailure:'any'};
test('native quick assertions and awaits retain skipped versus consumed dependencies',async t=>{
 const results=[];
 for(const config of ['tsconfig.json','loose.json','exact.json']) for(const marker of Object.keys(expected)) {
   const tag=`/*#${marker}*/`;
   const at={file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)};
   const answer=await typeOracle(path.join(root,config),{at});
   const hook=await compilerHookOracle(path.join(root,config),{operation:'type',at});
   assert.equal(answer.summary,hook.summary,`${config}/${marker}`);
   results.push({config,marker,summary:answer.summary,error:answer.errorType,codes:hook.beforeSummary.diagnosticCodes});
 }
 t.diagnostic(JSON.stringify(results));
 for(const result of results) {
   assert.equal(result.summary,expected[result.marker],`${result.config}/${result.marker}`);
   assert.equal(result.error,['awaitBad','constFailure'].includes(result.marker),`${result.config}/${result.marker}`);
   assert.deepEqual(result.codes,result.marker==='awaitBad'?[1320]:result.marker==='constFailure'?[1355,2304]:[],`${result.config}/${result.marker}`);
 }
});

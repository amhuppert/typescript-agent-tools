import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-call-type-arguments/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected={valid:['1',[]],rejected:['"wrong"',[2344]],hidden:['number',[2304]],later:['number',[2304,2344]],quick:['number',[]],untypedHidden:['any',[2304,2347]],untypedValid:['any',[2347]],spreadHidden:['number',[2304]],firstFailure:['number',[2304,2304]]};
test('native checked call type arguments consume source checks before candidate selection',async t=>{
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
   assert.equal(result.summary,expected[result.marker][0],`${result.config}/${result.marker}`);
   assert.equal(result.error,false,`${result.config}/${result.marker}`);
   assert.deepEqual(result.codes,expected[result.marker][1],`${result.config}/${result.marker}`);
 }
});

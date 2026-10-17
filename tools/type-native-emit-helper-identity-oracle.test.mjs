import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
import {assertFixtureCapture} from './type-oracle-fixture.mjs';
const root=path.resolve('fixtures/types/native-emit-helper-cache-resources/identity');
test('native per-file helper modules preserve a later signature recovery',async()=>{
 const rows=[];
 for(const variant of JSON.parse(fs.readFileSync(path.join(root,'variants.json'),'utf8'))) {
  const marker='/*#chosen*/',at={file:variant.file,...position(lineStarts(variant.source),variant.source.indexOf(marker)+marker.length)};
  const answer=await compilerHookOracle(path.join(root,variant.config),{operation:'type',at});
  rows.push({variant,at,answer});
  fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/native-emit-helper-identity.json',JSON.stringify(rows,null,2)+'\n');
  assert.equal(answer.compiler,'7.0.2');assert.equal(answer.checkerCount,1);
  assert.equal(answer.summary,'number');assert.equal(answer.errorType,false);
  const modules=answer.programFiles.filter(file=>file.file.endsWith('/node_modules/tslib/index.d.ts'));
  assert.equal(modules.length,2);
  const helpers=Object.values(answer.beforeSummary.observations).filter(event=>event.event==='helpers');
  assert.equal(helpers.length,2);assert.ok(helpers.every(event=>event.enabled&&event.helpers===8192));
  // As in native-private-access, helper signature recovery emits TS2304 while
  // retaining number; it is not a top-level hook demandFailure entry.
  assert.deepEqual(answer.demandFailures,[]);
  assert.deepEqual(answer.diagnosticCodes,variant.mode==='both-good'?[]:[2304]);
 }
 assert.equal(rows.length,12);
 const golden=path.join(root,'expected.json');if(fs.existsSync(golden))assertFixtureCapture(rows,JSON.parse(fs.readFileSync(golden,'utf8')),root);
});

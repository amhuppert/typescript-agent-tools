import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {assertFixtureCapture} from './type-oracle-fixture.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-emit-helper-cache-resources');
test('native inferred returns check per-file object-rest helper dependencies',async()=>{
 const rows=[];
 for(const variant of JSON.parse(fs.readFileSync(path.join(root,'variants.json'),'utf8'))) {
  const source=fs.readFileSync(path.join(root,variant.file),'utf8'),marker='/*#chosen*/';
  const at={file:variant.file,...position(lineStarts(source),source.indexOf(marker)+marker.length)};
  const answer=await compilerHookOracle(path.join(root,variant.config),{operation:'type',at});
  const dependencies=Array.from({length:variant.width},(_,index)=>{
   const file=`${variant.mode}-${index}.ts`;return {file,source:fs.readFileSync(path.join(root,file),'utf8')};
  });
  rows.push({variant,source,dependencies,at,answer});
  fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/native-emit-helper-cache-resources.json',JSON.stringify(rows,null,2)+'\n');
  assert.equal(answer.compiler,'7.0.2');assert.equal(answer.checkerCount,1);
  assert.equal(answer.summary,'number');assert.equal(answer.errorType,false);
  assert.deepEqual(answer.diagnosticCodes,[]);assert.deepEqual(answer.demandFailures,[]);
  for(let i=0;i<variant.width;i++) assert.ok(answer.programFiles.some(file=>file.file.endsWith(`/${variant.mode}-${i}.ts`)));
 }
 assert.equal(rows.length,16);
 const golden=path.join(root,'expected.json');if(fs.existsSync(golden))assertFixtureCapture(rows,JSON.parse(fs.readFileSync(golden,'utf8')),root);
});

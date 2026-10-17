import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {assertFixtureCapture} from './type-oracle-fixture.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-error-source-lookup-resources');
test('native recovery queries retain source ordering across wide unrelated inventories',async()=>{
 const rows=[];
 for(const variant of JSON.parse(fs.readFileSync(path.join(root,'variants.json'),'utf8'))) {
  const source=fs.readFileSync(path.join(root,variant.file),'utf8'),marker='/*#chosen*/';
  const at={file:variant.file,...position(lineStarts(source),source.indexOf(marker)+marker.length)};
  const answer=await compilerHookOracle(path.join(root,variant.config),{operation:'type',at});
  const starts=lineStarts(source);
  const callLocations=[...source.matchAll(/\(null! as Mixed\)\(\)/g)].map(match=>({start:position(starts,match.index),end:position(starts,match.index+match[0].length)}));
  const programOrder=answer.nativeProgramFiles.map(file=>path.relative(root,file.file).split(path.sep).join('/'));
  assert.deepEqual(programOrder,variant.configuration.files);
  assert.equal(callLocations.length,variant.width);
  rows.push({variant,source,at,answer,callLocations,programOrder});
  fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/native-error-source-lookup-resources.json',JSON.stringify(rows,null,2)+'\n');
  assert.equal(answer.compiler,'7.0.2');assert.equal(answer.checkerCount,1);
  assert.equal(answer.summary,variant.expected);assert.equal(answer.errorType,false);
  assert.deepEqual(answer.diagnosticCodes,variant.codes);assert.deepEqual(answer.demandFailures,[]);
 }
 assert.equal(rows.length,64);
 const golden=path.join(root,'expected.json');if(fs.existsSync(golden))assertFixtureCapture(rows,JSON.parse(fs.readFileSync(golden,'utf8')),root);
});

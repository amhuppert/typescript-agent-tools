import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {assertFixtureCapture} from './type-oracle-fixture.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-recovery-observation-resources');
test('native repeated unknown signatures retain their recovery value',async()=>{
 const rows=[];
 for(const variant of JSON.parse(fs.readFileSync(path.join(root,'variants.json'),'utf8'))) {
  const source=fs.readFileSync(path.join(root,variant.file),'utf8'),marker='/*#chosen*/';
  const at={file:variant.file,...position(lineStarts(source),source.indexOf(marker)+marker.length)};
  const answer=await compilerHookOracle(path.join(root,variant.config),{operation:'type',at});
  const starts=lineStarts(source);
  const callLocations=[...source.matchAll(/mixed\(\)/g)].map(match=>({start:position(starts,match.index),end:position(starts,match.index+match[0].length)}));
  assert.equal(callLocations.length,variant.width);
  rows.push({variant,source,at,answer,callLocations});
  fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/native-recovery-observation-resources.json',JSON.stringify(rows,null,2)+'\n');
  assert.equal(answer.compiler,'7.0.2');assert.equal(answer.checkerCount,1);
  assert.equal(answer.summary,variant.expected);assert.equal(answer.errorType,false);
  assert.deepEqual(answer.diagnosticCodes,variant.codes);assert.deepEqual(answer.demandFailures,[]);
 }
 assert.equal(rows.length,32);
 const golden=path.join(root,'expected.json');if(fs.existsSync(golden))assertFixtureCapture(rows,JSON.parse(fs.readFileSync(golden,'utf8')),root);
});

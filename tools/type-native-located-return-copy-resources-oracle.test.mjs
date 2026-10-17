import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {assertFixtureCapture} from './type-oracle-fixture.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-located-return-copy-resources');
test('native asserted calls replay a located union-return annotation',async()=>{
 const rows=[];
 for(const variant of JSON.parse(fs.readFileSync(path.join(root,'variants.json'),'utf8'))) {
  const source=fs.readFileSync(path.join(root,variant.file),'utf8'),marker='/*#chosen*/';
  const at={file:variant.file,...position(lineStarts(source),source.indexOf(marker)+marker.length)};
  const answer=await compilerHookOracle(path.join(root,variant.config),{operation:'type',at});
  const starts=lineStarts(source);
  const callLocations=[...source.matchAll(/\(null! as F\)\(\)/g)].map(match=>({start:position(starts,match.index),end:position(starts,match.index+match[0].length)}));
  assert.equal(callLocations.length,variant.width);
  const failureLocations=variant.missingNames.map(name=>{
   const offset=source.indexOf(name);assert.equal(source.split(name).length,2);
   return {start:position(starts,offset),end:position(starts,offset+name.length)};
  });
  rows.push({variant,source,at,answer,callLocations,failureLocations});
  fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/native-located-return-copy-resources.json',JSON.stringify(rows,null,2)+'\n');
  assert.equal(answer.compiler,'7.0.2');assert.equal(answer.checkerCount,1);
  assert.equal(answer.summary,variant.expected);assert.equal(answer.errorType,false);
  assert.deepEqual(answer.diagnosticCodes,variant.codes);assert.deepEqual(answer.demandFailures,[]);
 }
 assert.equal(rows.length,32);
 const golden=path.join(root,'expected.json');if(fs.existsSync(golden))assertFixtureCapture(rows,JSON.parse(fs.readFileSync(golden,'utf8')),root);
});

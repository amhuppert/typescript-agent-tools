import {assertFixtureCapture} from './type-oracle-fixture.mjs';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-reference-gap-resources');
test('native known member groups exclude unresolved same-spelled candidates',async()=>{
  const rows=[];
  for(const variant of JSON.parse(fs.readFileSync(path.join(root,'variants.json'),'utf8'))){
    const source=fs.readFileSync(path.join(root,variant.file),'utf8'),marker='/*#declaration*/';
    const answer=await compilerHookOracle(path.join(root,variant.config),{operation:'member',at:{file:variant.file,...position(lineStarts(source),source.indexOf(marker)+marker.length)}});
    rows.push({variant,source,answer});
    fs.mkdirSync('.cc/temp',{recursive:true});
    fs.writeFileSync('.cc/temp/native-reference-gap-resources.json',JSON.stringify(rows,null,2)+'\n');
    assert.equal(answer.compiler,'7.0.2');
    assert.equal(answer.checkerCount,1);
    assert.equal(answer.groups.length,1);
    assert.equal(answer.groups[0].occurrences.length,2);
    assert.deepEqual(answer.diagnosticCodes,Array(variant.width).fill(2304));
    assert.deepEqual(answer.demandFailures,[]);
  }
  assert.equal(rows.length,32);
  assertFixtureCapture(rows,JSON.parse(fs.readFileSync(path.join(root,'expected.json'),'utf8')),root);
});

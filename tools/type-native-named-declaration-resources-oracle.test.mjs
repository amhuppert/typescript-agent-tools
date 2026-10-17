import {assertFixtureCapture} from './type-oracle-fixture.mjs';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-named-declaration-resources');
test('native merged overload declarations retain exact Unicode names and file ordering',async()=>{
 const rows=[];
 for(const variant of JSON.parse(fs.readFileSync(path.join(root,'variants.json'),'utf8'))){
  const source=fs.readFileSync(path.join(root,variant.file),'utf8'),marker='/*#chosen*/';
  const answer=await compilerHookOracle(path.join(root,variant.config),{operation:'member',at:{file:variant.file,...position(lineStarts(source),source.indexOf(marker)+marker.length)}});
  rows.push({variant,source,answer});
  fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/native-named-declaration-resources.json',JSON.stringify(rows,null,2)+'\n');
  assert.equal(answer.compiler,'7.0.2');assert.equal(answer.checkerCount,1);
  assert.deepEqual(answer.diagnosticCodes,[]);assert.deepEqual(answer.demandFailures,[]);
  assert.equal(answer.rawOrigins.length,variant.width);
  // The normalized baseline reads the raw parent's own member table. For a
  // cross-file checker merge, that selects the first file's unmerged member;
  // the compiler's directional search produces no entries for that seed.
  assert.equal(answer.combinedOrigins.length,variant.layout === 'split' ? variant.width/2 : variant.width);
  assert.equal(answer.groups.length,variant.layout === 'split' ? 0 : 1);
  if(variant.layout === 'single') assert.equal(answer.groups[0].occurrences.length,variant.width+1);
 }
 assert.equal(rows.length,16);
 const golden=path.join(root,"expected.json");
 if(fs.existsSync(golden)) assertFixtureCapture(rows,JSON.parse(fs.readFileSync(golden,"utf8")),root);
});

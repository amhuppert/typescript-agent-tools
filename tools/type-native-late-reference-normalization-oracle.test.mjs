import {assertFixtureCapture} from './type-oracle-fixture.mjs';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-late-merge-resources');
test('normalized early and late seeds retain native candidate identities',async()=>{
 const rows=[];
 for(const profile of ['strict','nocheck','loose','loose-nocheck']) for(const width of [16,64]) {
  const file=`${width}.ts`,source=fs.readFileSync(path.join(root,file),'utf8');
  for(const [seed,needle,skip,occurrences] of [
   ['only-late-use','item.method',5,1],
   ['mixed-use','early.method',6,2],
   ['mixed-declaration','interface EarlyShape { method',23,2],
  ]) {
   const offset=source.indexOf(needle);assert.notEqual(offset,-1,needle);
   const at={file,...position(lineStarts(source),offset+skip)};
   const answer=await compilerHookOracle(path.join(root,`${profile}.json`),{operation:'member',at});
   // Raw text search can omit computed-only uses. The documented decoded-key
   // normalization uses the same checker's established candidate relationships.
   const related=answer.candidateObservations.filter(row=>row.relatedOrigins.length>0);
   const normalizedGroups=related.length ? [{origins:related[0].relatedOrigins,occurrences:related.map(row=>row.nativeRange)}] : [];
   rows.push({profile,width,seed,at,answer,normalizedGroups});
   fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/native-late-reference-normalization.json',JSON.stringify(rows,null,2)+'\n');
   assert.equal(answer.compiler,'7.0.2');assert.equal(answer.checkerCount,1);
   assert.deepEqual(answer.diagnosticCodes,[]);assert.deepEqual(answer.demandFailures,[]);
   const declarationCount=width+(seed==='only-late-use'?0:1);
   assert.equal(answer.combinedOrigins.length,declarationCount,`${profile}/${width}/${seed}`);
   assert.equal(normalizedGroups.length,1,`${profile}/${width}/${seed}`);
   assert.equal(normalizedGroups[0].origins.length,declarationCount);
   assert.equal(normalizedGroups[0].occurrences.length,occurrences,`${profile}/${width}/${seed}`);
   for(const row of related) {
    assert.equal(row.inDomain,true);
    assert.deepEqual(row.relatedOrigins,normalizedGroups[0].origins);
   }
  }
 }
 assert.equal(rows.length,24);
 const golden=path.join(root,'expected-references.json');
 if(fs.existsSync(golden))assertFixtureCapture(rows,JSON.parse(fs.readFileSync(golden,'utf8')),root);
});

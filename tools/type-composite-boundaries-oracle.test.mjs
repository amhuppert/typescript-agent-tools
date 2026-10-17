import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-composite-boundaries/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
for (const profile of ['tsconfig.json','loose.json']) {
 test(`native composite predicates, mixins and array fallback (${profile})`,async()=>{
  for (const [marker,summary] of [
   ['predicate','"chosen"'],['falsePredicate','"fallback"'],['booleanPredicate','"fallback"'],
   ['assertionPredicate','"fallback"'],['mismatchPredicate','"fallback"'],
   ['mixin','"chosen"'],['allMixins','"chosen"'],['readonlyMixin','"chosen"'],
   ['mutableArrays','(string | number)[]'],['readonlyArrays','readonly (string | number)[]'],
   ['mixedArrays','readonly (string | number)[]'],
  ]) {
   const tag=`/*#${marker}*/`;
   const answer=await typeOracle(path.join(root,profile),{at:{file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)},properties:false});
   assert.equal(answer.compiler,'7.0.2');
   assert.equal(answer.errorType,false,`${marker}: ${JSON.stringify(answer)}`);
   assert.equal(answer.summary,summary,`${marker}: ${JSON.stringify(answer)}`);
  }
 });
}

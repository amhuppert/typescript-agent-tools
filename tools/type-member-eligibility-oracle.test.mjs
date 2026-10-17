import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';

const root=fileURLToPath(new URL('../fixtures/types/member-reference-eligibility/',import.meta.url));
function at(file,marker){
 const source=fs.readFileSync(path.join(root,file),'utf8'),tag=`/*#${marker}*/`;
 const offset=source.indexOf(tag);
 assert.notEqual(offset,-1,marker);
 return {file,...position(lineStarts(source),offset+tag.length)};
}
function observation(answer,file,marker){
 const target=at(file,marker);
 const result=answer.candidateObservations.find(row=>row.location.file===file&&row.location.start.line===target.line&&row.location.start.column===target.column);
 assert.ok(result,`${file}/${marker}`);
 return result;
}

test('native reference eligibility and location meanings preserve all admitted syntax roles',async()=>{
 const rows=[];
 for(const [file,marker] of [['globals.d.ts','lengthDeclaration'],['cases.ts','constNamespace'],['cases.ts','valueNamespace'],['globals.d.ts','ambientDeclaration']]){
  const answer=await compilerHookOracle(path.join(root,'tsconfig.json'),{operation:'member',at:at(file,marker)});
  rows.push({file,marker,answer});
  fs.writeFileSync('.cc/temp/member-eligibility-native.json',JSON.stringify(rows,null,2));
 }
 for(const {marker,answer} of rows){
  assert.equal(answer.compiler,'7.0.2');
  assert.deepEqual(answer.demandFailures,[],marker);
  if(marker==='lengthDeclaration'){
   for(const marker of ['objectLiteral','indexLiteral','importLiteral','exportLiteral','defineProperty'])
    assert.equal(observation(answer,'cases.ts',marker).validRaw,true,marker);
   for(const marker of ['ordinaryLiteral','definePropertyWrongArity','definePropertyParenthesized'])
    assert.equal(observation(answer,'cases.ts',marker).validRaw,false,marker);
   for(const marker of ['objectLiteral','indexLiteral']){
    const row=observation(answer,'cases.ts',marker);
    assert.equal(row.meaning,3,marker);
    assert.equal(row.relatedOrigins.length,1,marker);
    assert.equal(row.relatedOrigins[0].location.file,'globals.d.ts',marker);
   }
  } else if(marker==='constNamespace'){
   assert.equal(observation(answer,'cases.ts','constNamespace').meaning,4);
   assert.equal(observation(answer,'cases.ts','constTypeUse').meaning,4);
   assert.equal(observation(answer,'cases.ts','constValueUse').meaning,1);
  } else if(marker==='valueNamespace'){
   assert.equal(observation(answer,'cases.ts','valueNamespace').meaning,5);
   assert.equal(observation(answer,'cases.ts','liveTypeUse').meaning,4);
   assert.equal(observation(answer,'cases.ts','liveValueUse').meaning,1);
  } else {
   assert.equal(observation(answer,'globals.d.ts','ambientDeclaration').meaning,5);
   assert.equal(observation(answer,'cases.ts','ambientImport').meaning,7);
   assert.equal(observation(answer,'cases.ts','ambientImport').validRaw,true);
  }
 }
});

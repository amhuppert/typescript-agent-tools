import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle,typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-qualified-alias-boundaries/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected=[
 ['directDefault','Defaulted<string>',[['value','string',false]]],
 ['namedDefault','Defaulted',[['value','string',false]]],
 ['starDefault','Defaulted',[['value','string',false]]],
 ['directPartial','Pair<boolean, number>',[['left','boolean',false],['right','number',false]]],
 ['namedPartial','Pair<boolean>',[['left','boolean',false],['right','number',false]]],
 ['json','JsonShadow<"json">',[['inner','"json"',false]]],
 ['hidden','Carrier<"hidden">',[['nested','External<"hidden">',false]]],
 ['broken','Broken<"broken">',[['broken','MissingHidden',true]]],
];
test('native qualified alias defaults, value-target negatives and unsearched property dependencies',async()=>{
 const observations=[];
 for(const config of ['tsconfig.json','loose.json','exact.json']) for(const [marker] of expected){
  const tag=`/*#${marker}*/`,at={file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)};
  const answer=await typeOracle(path.join(root,config),{at,properties:true});
  const hook=await compilerHookOracle(path.join(root,config),{at});
  observations.push({config,marker,summary:answer.summary,errorType:answer.errorType,properties:answer.properties.map(p=>[p.name,p.summary,p.errorType]),hook});
  fs.writeFileSync(scratchPath('native-qualified-alias-boundaries-observations.json'),JSON.stringify(observations,null,2));
 }
 for(const row of observations){
  const [,summary,properties]=expected.find(([marker])=>marker===row.marker);
  assert.equal(row.summary,summary,`${row.config}/${row.marker}`);
  assert.equal(row.errorType,false,`${row.config}/${row.marker}`);
  assert.deepEqual(row.properties,properties,`${row.config}/${row.marker}`);
  assert.equal(row.hook.summary,row.summary,`${row.config}/${row.marker} root before member demands`);
 }
});

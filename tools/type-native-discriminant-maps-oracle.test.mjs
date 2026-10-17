import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle,typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-discriminant-maps/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected=JSON.parse(fs.readFileSync(path.join(root,'expected.json'),'utf8'));
for(const config of ['tsconfig','exact','loose']) test(`native discriminant maps (${config})`,async t=>{
 const rows=[];
 for(const row of expected){
  const marker=`/*#${row.name}*/`;
  const at={file:'cases.ts',...position(lineStarts(source),source.indexOf(marker)+marker.length)};
  const answer=await typeOracle(path.join(root,config+'.json'),{at,properties:false});
  const hook=await compilerHookOracle(path.join(root,config+'.json'),{at});
  assert.equal(answer.compiler,'7.0.2');
  assert.equal(answer.summary,hook.summary,row.name);
  assert.equal(answer.summary,'"matched"',row.name);
  assert.equal(answer.errorType,false,row.name);
  const maps=Object.entries(hook.beforeSummary.observations).filter(([key])=>key.startsWith('unionKey:')).map(([,value])=>value);
  assert.deepEqual(maps,row.maps,`${row.name}: native map installation`);
  assert.deepEqual(hook.beforeSummary.diagnosticCodes,row.codes,`${row.name}: consumed dependencies`);
  rows.push({name:row.name,summary:answer.summary,errorType:answer.errorType,codes:hook.beforeSummary.diagnosticCodes,maps});
 }
 fs.mkdirSync('.cc/temp',{recursive:true});
 fs.writeFileSync(`.cc/temp/native-discriminant-maps-${config}.json`,JSON.stringify(rows,null,2));
 t.diagnostic(`Asserted ${rows.length} native map boundaries`);
});

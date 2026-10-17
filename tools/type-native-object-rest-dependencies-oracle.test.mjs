import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle,compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-object-rest/',import.meta.url));
test('native rest generic aliases and imported helpers retain actual dependency gates',async()=>{
 const rows=[];
 for(const config of ['generic-missing','generic-arity','generic-kind','generic-broken','generic-mapped','helpers/missing','helpers/exact','helpers/wildcard','helpers/type-only','helpers/broken','helpers/script','helpers/disabled','helpers/newer','helpers/script-source','helpers/isolated']){
  const file=config.startsWith('generic-')?'generic.ts': ['helpers/script-source','helpers/isolated'].includes(config)?'script.ts':'cases.ts';
  const source=fs.readFileSync(path.join(root,path.dirname(config),file),'utf8');
  for(const match of source.matchAll(/\/\*#([^*]+)\*\//g)){
   const at={file,...position(lineStarts(source),match.index+match[0].length)};
   const answer=await typeOracle(path.join(root,config+'.json'),{at,properties:false});
   const hook=await compilerHookOracle(path.join(root,config+'.json'),{operation:'type',at});
   assert.equal(answer.compiler,'7.0.2');assert.equal(answer.summary,hook.summary);
   rows.push({config,name:match[1],summary:answer.summary,errorType:answer.errorType,codes:hook.beforeSummary.diagnosticCodes});
  }
 }
 fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/types-object-rest-dependencies-native.json',JSON.stringify(rows,null,2));
 assert.deepEqual(rows,JSON.parse(fs.readFileSync(path.join(root,'expected-dependencies.json'),'utf8')));
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle,compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-object-rest/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
test('native object rest preserves symbol copying, filters and actual Omit acquisition',async()=>{
 const rows=[];
 for(const config of ['tsconfig','loose','exact']) for(const match of source.matchAll(/\/\*#([^*]+)\*\//g)){
  const at={file:'cases.ts',...position(lineStarts(source),match.index+match[0].length)};
  const answer=await typeOracle(path.join(root,config+'.json'),{at,properties:false});
  const hook=await compilerHookOracle(path.join(root,config+'.json'),{operation:'type',at});
  assert.equal(answer.compiler,'7.0.2');assert.equal(answer.summary,hook.summary);
  rows.push({config,name:match[1],summary:answer.summary,errorType:answer.errorType,codes:hook.beforeSummary.diagnosticCodes});
 }
 fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/types-object-rest-native.json',JSON.stringify(rows,null,2));
 assert.deepEqual(rows,JSON.parse(fs.readFileSync(path.join(root,'expected.json'),'utf8')));
});

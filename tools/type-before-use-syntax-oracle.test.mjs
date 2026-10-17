import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-before-use-syntax/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const cases=[['earlier','number'],['deferred','number'],['iife','number'],['asyncIife','number'],['generatorIife','number'],['arrow','number'],['shadow','number'],['computed','"a"'],['self','number'],['selfArrow','number'],['parameterProperty','number'],['staticArrow','number'],['optional','number | undefined'],['asserted','number'],['nested','number'],['method','() => number'],['inherited','number'],['decorator','number'],['decoratorArrow','number']];
test('native complete before-use syntax families retain their actual final types and ordered questions',async()=>{
 const observations=[];
 for(const config of ['tsconfig.json','legacy.json','loose.json']) for(const [marker,summary] of cases){
  const label=`/*#${marker}*/`; const offset=source.indexOf(label)+label.length;
  const answer=await compilerHookOracle(path.join(root,config),{at:{file:'cases.ts',...position(lineStarts(source),offset)},properties:false});
  observations.push({config,marker,summary:answer.summary,questions:Object.entries(answer.beforeSummary.observations).filter(([key])=>key.startsWith('beforeUse:')).map(([,value])=>value)});
  fs.writeFileSync('.cc/temp/native-before-use-syntax.json',JSON.stringify(observations,null,2));
  assert.equal(answer.summary,config==='loose.json'&&marker==='optional'?'number':summary,`${config}/${marker}`);
  assert.equal(answer.errorType,false,`${config}/${marker}`);
 }
 assert.equal(observations.length,cases.length*3);
});

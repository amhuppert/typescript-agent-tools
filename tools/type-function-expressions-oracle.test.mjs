import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-function-expressions/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const cases=[['ordinary','(value: number) => number'],['named','(value: string) => string'],['arrow','(value: number) => number'],['generic','<T>(value: T) => T'],['initialized','(value?: number) => number'],['rest','(...values: string[]) => string'],['iife','number'],['context','number'],['contextParameter','string'],['contextThis','number'],['constantPredicate','(x: number) => true'],['constantComparison','(x: number) => boolean']];
test('native function expression factories and contextual assignment retain final values',async()=>{
 const observations=[];
 for(const config of ['tsconfig.json','loose.json','exact.json']) for(const [marker,summary] of cases){
  const label=`/*#${marker}*/`;const offset=source.indexOf(label)+label.length;
  const answer=await typeOracle(path.join(root,config),{at:{file:'cases.ts',...position(lineStarts(source),offset)},properties:false});
  observations.push({config,marker,answer});fs.writeFileSync(scratchPath('native-function-expressions.json'),JSON.stringify(observations,null,2));
  assert.equal(answer.summary,config==='loose.json'&&marker==='initialized'?'(value?: number) => number':summary,`${config}/${marker}`);
 }
});

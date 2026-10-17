import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-intrinsic-undefined/',import.meta.url));
test('native undefined intrinsic identity, shadows and aliases',async()=>{
 const observations=[];
 const questions=[];
 for(const config of ['tsconfig.json','loose.json','exact.json']) for(const marker of ['direct','constant','automatic','automaticRead','parenthesized','queried','shadow','shadowVariable','localTypeOnly','assignment','imported','declaredAlias']) questions.push({config,marker,file:['imported','declaredAlias'].includes(marker)?'consumer.ts':'cases.ts'});
 for(const [marker,config,file] of [['globalType','global-type.json','global-type.ts'],['globalValue','global-value.json','global-value.ts']]) questions.push({marker,config,file});
 for(const question of questions){
  const source=fs.readFileSync(path.join(root,question.file),'utf8');const label=`/*#${question.marker}*/`;const offset=source.indexOf(label)+label.length;
  const answer=await typeOracle(path.join(root,question.config),{at:{file:question.file,...position(lineStarts(source),offset)},properties:false});
  observations.push({...question,answer});fs.writeFileSync('.cc/temp/native-intrinsic-undefined.json',JSON.stringify(observations,null,2));
  assert.equal(answer.compiler,'7.0.2');
  const loose=question.config==='loose.json';
  const expected={direct:'undefined',constant:loose?'any':'undefined',automatic:'any',automaticRead:loose?'any':'number',parenthesized:'any',queried:loose?'any':'undefined',shadow:'number',shadowVariable:'number',localTypeOnly:loose?'any':'undefined',assignment:'any',imported:'any',declaredAlias:'1',globalType:'any',globalValue:'string'};
  assert.deepEqual([answer.summary,answer.errorType],[expected[question.marker],['assignment','imported','globalType'].includes(question.marker)],`${question.config}/${question.marker}`);
 }
});

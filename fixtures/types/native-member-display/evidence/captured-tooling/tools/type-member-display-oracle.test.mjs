import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory=fileURLToPath(new URL('../fixtures/types/native-member-display/',import.meta.url));
const source=fs.readFileSync(path.join(directory,'cases.ts'),'utf8');
const options=['strict.json','exact.json','loose.json'];
const at=marker => {
  const token=`/*#${marker}*/ `;
  const offset=source.indexOf(token);
  assert.notEqual(offset,-1,marker);
  return {file:'cases.ts',...position(lineStarts(source),offset+token.length)};
};
async function question(config,marker){
  const answer=await typeOracle(path.join(directory,config),{at:at(marker)});
  assert.equal(answer.compiler,'7.0.2');
  assert.equal(answer.kind,'value');
  assert.equal(answer.errorType,false);
  assert.deepEqual(answer.prelude,[]);
  return answer;
}

test('native inline optional property serialization and raw property reads are distinct questions',async()=>{
  for(const config of options){
    const answer=await question(config,'property');
    assert.equal(answer.summary,config==='strict.json'?'{ value?: string | undefined; }':'{ value?: string; }');
    assert.equal(answer.properties[0].summary,config==='loose.json'?'string':'string | undefined');
    assert.equal(answer.properties[0].errorType,false);
    assert.equal(answer.properties[0].origins[0].kind,'PropertySignature');
  }
});

test('native optional method serialization preserves method syntax independently of raw callable reads',async()=>{
  for(const config of options){
    const answer=await question(config,'method');
    assert.equal(answer.summary,'{ value?(): number; }');
    assert.equal(answer.properties[0].summary,config==='loose.json'?'() => number':'(() => number) | undefined');
    assert.equal(answer.properties[0].origins[0].kind,'MethodSignature');
  }
});

test('native inline distinct accessor serialization uses both actual signature questions',async()=>{
  for(const config of options){
    const answer=await question(config,'accessor');
    assert.equal(answer.summary,'{ get value(): string; set value(next: number); }');
    assert.equal(answer.properties[0].summary,'string');
    assert.deepEqual(answer.properties[0].origins.map(origin=>origin.kind),['GetAccessor','SetAccessor']);
  }
});

test('native inline accessor summary consumes the erroneous setter even when the displayed root stays healthy',async()=>{
  const setterAt={file:'cases.ts',...position(lineStarts(source),source.indexOf('next: Missing'))};
  for(const config of options){
    const answer=await question(config,'ignoredBadSetter');
    assert.equal(answer.summary,'{ value: string; }');
    assert.equal(answer.properties[0].summary,'string');
    const child=await typeOracle(path.join(directory,config),{at:setterAt,properties:false});
    assert.equal(child.kind,'value');
    assert.equal(child.summary,'Missing');
    assert.equal(child.errorType,true);
    const hook=await compilerHookOracle(path.join(directory,config),{at:at('ignoredBadSetter'),operation:'type'});
    assert.equal(hook.summary,answer.summary);
    assert.equal(hook.errorType,false);
    assert.ok(hook.beforeSummary.diagnosticCodes.length>0);
    assert.ok(hook.beforeSummary.diagnosticCodes.every(code=>code===2318));
    assert.deepEqual(hook.diagnosticCodes,[...hook.beforeSummary.diagnosticCodes,2304]);
    assert.deepEqual(hook.beforeSummary.demandFailures,[]);
    assert.deepEqual(hook.demandFailures,[],'limit events do not represent ordinary error-type provenance');
  }
});

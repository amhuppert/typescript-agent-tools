import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle,compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';

for (const config of ['tsconfig.json','loose.json','exact.json']) {
  test(`native object member/context/spread grammar ${config}`,async()=>{
    const root=fileURLToPath(new URL('../fixtures/types/native-object-literals/',import.meta.url));
    const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
    const rows=[];
    for(const match of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at={file:'cases.ts',...position(lineStarts(source),match.index+match[0].length)};
      const answer=await typeOracle(path.join(root,config),{at,properties:false});
      const hook=await compilerHookOracle(path.join(root,config),{operation:'type',at});
      assert.equal(answer.compiler,'7.0.2');
      assert.equal(answer.summary,hook.summary);
      rows.push({name:match[1],summary:answer.summary,errorType:answer.errorType,codes:hook.beforeSummary.diagnosticCodes});
    }
    fs.mkdirSync('.cc/temp',{recursive:true});
    fs.writeFileSync(`.cc/temp/types-object-literals-${config}`,JSON.stringify(rows,null,2)+'\n');
    const expected=path.join(root,`expected-${config}`);
    if(fs.existsSync(expected))assert.deepEqual(rows,JSON.parse(fs.readFileSync(expected,'utf8')));
    else assert.equal(rows.length,25);
  });
}

test('native duplicate-accessor grammar admission',async()=>{
  const root=fileURLToPath(new URL('../fixtures/types/native-object-literals/',import.meta.url));
  const rows=[];
  for(const name of ['duplicateGetter','duplicateSetter','propertyAccessor','accessorProperty','methodProperty']) {
    const source=fs.readFileSync(path.join(root,`${name}.ts`),'utf8');
    const marker=`/*#${name}*/`;
    const at={file:`${name}.ts`,...position(lineStarts(source),source.indexOf(marker)+marker.length)};
    let answer;
    try { answer=await typeOracle(path.join(root,`${name}.json`),{at,properties:false}); }
    catch(error) {
      if(!error.message.startsWith('TypeScript rejected the project:'))throw error;
      rows.push({name,rejected:error.message});
      continue;
    }
    const hook=await compilerHookOracle(path.join(root,`${name}.json`),{operation:'type',at});
    rows.push({name,summary:answer.summary,errorType:answer.errorType,codes:hook.beforeSummary.diagnosticCodes});
  }
  fs.writeFileSync('.cc/temp/types-object-literals-grammar.json',JSON.stringify(rows,null,2)+'\n');
  const expected=path.join(root,'expected-grammar.json');
  if(fs.existsSync(expected))assert.deepEqual(rows,JSON.parse(fs.readFileSync(expected,'utf8')));
  else assert.equal(rows.length,5);
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';
const hash=text=>createHash('sha256').update(text).digest('hex');

for(const bom of ['', '\uFEFF'])test('native replay publishes its actual ordered sources and provenance (BOM='+Boolean(bom)+')',async t=>{
  const root=fs.realpathSync(fs.mkdtempSync(scratchPath('native-program-inventory-')));
  t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
  const a='export const answer=42;\n';
  const z='import {answer} from "./a";\nexport const query=answer;\n';
  fs.writeFileSync(path.join(root,'a.ts'),bom+a);
  fs.writeFileSync(path.join(root,'z.ts'),bom+z);
  fs.writeFileSync(path.join(root,'tsconfig.json'),JSON.stringify({compilerOptions:{strict:true,noLib:true,noEmit:true},files:['z.ts','a.ts']}));
  const answer=await compilerHookOracle(path.join(root,'tsconfig.json'),{at:{file:'z.ts',line:2,column:14}});
  assert.equal(answer.summary,'42');
  assert.equal(answer.errorType,false);
  assert.deepEqual(answer.nativeProgramFiles,[
    {file:path.join(root,'a.ts'),defaultLibrary:false,externalLibrary:false,sourceTextSha256:hash(a)},
    {file:path.join(root,'z.ts'),defaultLibrary:false,externalLibrary:false,sourceTextSha256:hash(z)},
  ],'raw inventory must come from the replay program, not the API inventory supplied as input');
  assert.deepEqual(answer.programFiles,answer.nativeProgramFiles.map(({sourceTextSha256,...row})=>row));
  assert.deepEqual(answer.apiProgramFiles,answer.programFiles);
  assert.equal(answer.inputHashes.find(row=>row.file===path.join(root,'a.ts')).sha256,hash(bom+a),
    'decoded native source evidence must not replace raw input byte hashes');
});

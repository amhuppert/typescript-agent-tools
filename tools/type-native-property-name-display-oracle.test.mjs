import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle,compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';

test('native property-name declaration and nameType rendering',async()=>{
  const root=fileURLToPath(new URL('../fixtures/types/native-property-name-display/',import.meta.url));
  const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
  const rows=[];
  for(const match of source.matchAll(/\/\*#([^*]+)\*\//g)) {
    const at={file:'cases.ts',...position(lineStarts(source),match.index+match[0].length)};
    const answer=await typeOracle(path.join(root,'tsconfig.json'),{at,properties:false});
    const hook=await compilerHookOracle(path.join(root,'tsconfig.json'),{operation:'type',at});
    assert.equal(answer.compiler,'7.0.2');
    assert.equal(answer.summary,hook.summary);
    rows.push({name:match[1],summary:answer.summary,errorType:answer.errorType,codes:hook.beforeSummary.diagnosticCodes});
  }
  fs.mkdirSync('.cc/temp',{recursive:true});
  fs.writeFileSync('.cc/temp/types-property-name-display-native.json',JSON.stringify(rows,null,2)+'\n');
  const expected=path.join(root,'expected.json');
  if(fs.existsSync(expected))assert.deepEqual(rows,JSON.parse(fs.readFileSync(expected,'utf8')));
  else assert.equal(rows.length,12);
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-symbol-display/',import.meta.url));
test('native default SymbolToString preserves raw symbols and written names without acquiring value types',async()=>{
 const rows=[];
 for(const file of ['cases.ts','modules.d.ts']) {
 const source=fs.readFileSync(path.join(root,file),'utf8');
 for(const match of source.matchAll(/\/\*#([^*]+)\*\//g)){
  const answer=await compilerHookOracle(path.join(root,'tsconfig.json'),{history:'cold',inspectSymbolDisplay:true,at:{file,...position(lineStarts(source),match.index+match[0].length)}});
  assert.equal(answer.compiler,'7.0.2');
  assert.equal(typeof answer.symbolDisplayBytes,'string',match[1]);
  rows.push({name:match[1],nodeKind:answer.symbolDisplayNodeKind,display:Buffer.from(answer.symbolDisplayBytes,'base64').toString('utf8'),bytes:Buffer.from(answer.symbolDisplayBytes,'base64').length,beforeCodes:answer.beforeSymbolDisplay.diagnosticCodes,afterCodes:answer.diagnosticCodes,failures:answer.demandFailures});
 }
 }
 fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/types-symbol-display-native.json',JSON.stringify(rows,null,2));
 assert.deepEqual(rows,JSON.parse(fs.readFileSync(path.join(root,'expected.json'),'utf8')));
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-diagnostic-bytes/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
test('native final diagnostic truncation preserves exact bytes even across Unicode code points',async t=>{
 const rows=[];
 for(const config of ['tsconfig.json','no-truncation.json']){
  for(const match of source.matchAll(/declare const \/\*#([^*]+)\*\/[^:]+: ([^;]+);/g)){
   const name=match[1],written=Buffer.from(match[2],'utf8');
   const tag=`/*#${name}*/`,offset=source.indexOf(tag)+tag.length;
   const at={file:'cases.ts',...position(lineStarts(source),offset)};
   const answer=await compilerHookOracle(path.join(root,config),{operation:'type',at,inspectSummaryBytes:true});
   assert.equal(answer.compiler,'7.0.2');
   assert.equal(typeof answer.summaryBytes,'string',`${config}/${name}/raw observation required`);
   const bytes=Buffer.from(answer.summaryBytes,'base64');
   const expected=config==='tsconfig.json'&&written.length>=320
    ?Buffer.concat([written.subarray(0,317),Buffer.from('...')]):written;
   assert.deepEqual(bytes,expected,`${config}/${name}/exact native bytes`);
   assert.equal(answer.errorType,false,`${config}/${name}`);
   assert.deepEqual(answer.beforeSummary.diagnosticCodes,[],`${config}/${name}`);
   assert.deepEqual(answer.demandFailures,[],`${config}/${name}`);
   let validUtf8=true;try{new TextDecoder('utf-8',{fatal:true}).decode(bytes);}catch{validUtf8=false;}
   const split=['twoByteSplit','threeByteSplit','fourByteFirst','fourByteSecond','fourByteThird'].includes(name)&&config==='tsconfig.json';
   assert.equal(validUtf8,!split,`${config}/${name}/UTF-8 validity`);
   rows.push({config,name,inputBytes:written.length,outputBytes:bytes.length,validUtf8,tailHex:bytes.subarray(-11).toString('hex')});
  }
 }
 fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/types-diagnostic-bytes-native.json',JSON.stringify(rows,null,2));
 t.diagnostic(JSON.stringify(rows));
});

test('explicit NoTruncation formats the whole native type without changing compiler options', async () => {
 const match = [...source.matchAll(/declare const \/\*#([^*]+)\*\/[^:]+: ([^;]+);/g)].find(match => Buffer.byteLength(match[2]) > 320);
 assert.ok(match);
 const tag = `/*#${match[1]}*/`;
 const answer = await compilerHookOracle(path.join(root, 'tsconfig.json'), {
   at: {file:'cases.ts', ...position(lineStarts(source), source.indexOf(tag)+tag.length)},
   inspectSummaryBytes:true, noTruncation:true,
 });
 assert.deepEqual(Buffer.from(answer.summaryBytes, 'base64'), Buffer.from(match[2]));
 assert.equal(answer.errorType, false);
});

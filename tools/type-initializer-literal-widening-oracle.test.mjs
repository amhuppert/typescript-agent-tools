import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';

// The answers tests/type_initializer_literal_widening.rs expects, as the
// pinned TypeScript 7.0.2 checker gives them. A parameter property is not
// `isDeclarationReadonly`, so its initializer's fresh literal widens; a
// readonly class property keeps it. The compiler hook answers both members
// of a parameter-property symbol pair.
const fixtures=fileURLToPath(new URL('../fixtures/types/',import.meta.url));
const root=path.join(fixtures,'native-initializer-literal-widening');
const at=(file,line,column,length)=>({file,start:{line,column},end:{line,column:column+length}});

test('initializer literals widen for every source and declaration as native does',async()=>{
 const rows=[];
 for(const file of ['cases.ts','calls.ts']){
  const source=fs.readFileSync(path.join(root,file),'utf8');
  for(const match of source.matchAll(/\/\*#([^*]+)\*\//g)){
   const answer=await compilerHookOracle(path.join(root,'tsconfig.json'),{operation:'type',at:{file,...position(lineStarts(source),match.index+match[0].length)}});
   assert.equal(answer.compiler,'7.0.2');
   assert.deepEqual(answer.demandFailures,[],match[1]);
   assert.deepEqual(answer.diagnosticCodes,[],match[1]);
   rows.push({file,name:match[1],summary:answer.summary,errorType:answer.errorType});
  }
 }
 assert.deepEqual(rows,JSON.parse(fs.readFileSync(path.join(root,'expected.json'),'utf8')));
});

test('a member search through the widened parameter property call has every occurrence',async()=>{
 const forms=await compilerHookOracle(path.join(root,'tsconfig.json'),{operation:'member',at:{file:'cases.ts',line:79,column:3}});
 assert.deepEqual(forms.demandFailures,[]);
 assert.deepEqual(forms.groups,[{origins:[{kind:'MethodDeclaration',location:at('cases.ts',79,3,8)}],occurrences:[at('calls.ts',10,57,8),at('cases.ts',79,3,8)]}]);
 const call=path.join(fixtures,'native-initializer-literal-widening-call','tsconfig.json');
 for(const [file,line,column,summary] of [['a.ts',2,47,'string'],['b.ts',3,18,'A']]){
  const answer=await compilerHookOracle(call,{operation:'type',at:{file,line,column}});
  assert.equal(answer.summary,summary,`${file}:${line}:${column}`);
  assert.deepEqual(answer.diagnosticCodes,[]);
 }
 const get=await compilerHookOracle(call,{operation:'member',at:{file:'a.ts',line:2,column:59}});
 assert.deepEqual(get.demandFailures,[]);
 assert.deepEqual(get.groups,[{origins:[{kind:'MethodDeclaration',location:at('a.ts',2,59,3)}],occurrences:[at('a.ts',2,59,3),at('b.ts',4,24,3)]}]);
});

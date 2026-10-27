import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { lineStarts, position } from './semantic-oracle.mjs';
import { typeOracle, compilerHookOracle } from './type-oracle.mjs';
import { scratchPath } from './harness-scratch.mjs';

const fixture=fileURLToPath(new URL('../fixtures/types/selection-review/',import.meta.url));
const config=path.join(fixture,'tsconfig.json');
function at(marker,file='cases.ts') {
 const source=fs.readFileSync(path.join(fixture,file),'utf8');
 const text=`/*#${marker}*/`,offset=source.indexOf(text);
 assert.notEqual(offset,-1,marker);
 return {file,...position(lineStarts(source),offset+text.length)};
}
const request=marker=>({at:at(marker),properties:false});

test('Q4 renamed type-only alias sides both ask the followed target declared question',async()=>{
 const expected=await typeOracle(config,{...request('typeClassRight'),question:'declared'});
 assert.equal(expected.summary,'Widget');
 assert.equal(expected.errorType,false);
 const actual=await typeOracle(config,request('typeClassLeft'));
 assert.equal(actual.kind,'declared','renamed source side must share the alias site question');
 assert.equal(actual.summary,expected.summary);
 assert.equal(actual.errorType,false);
});

test('Q4 type-query operand asks a value question instead of the containing type-syntax question',async()=>{
 const expected=await typeOracle(config,{...request('typeQuery'),question:'value'});
 assert.equal(expected.summary,'typeof Local');
 assert.equal(expected.errorType,false);
 const actual=await typeOracle(config,request('typeQuery'));
 assert.equal(actual.kind,'value');
 assert.equal(actual.summary,expected.summary);
 assert.equal(actual.errorType,false);
});

test('Q4 parameter-property modifier selects its member value role in the same-checker control',async()=>{
 const source=fs.readFileSync(path.join(fixture,'cases.ts'),'utf8');
 const ordinaryOffset=source.indexOf('function call(value: number)')+'function call('.length;
 const ordinary=await typeOracle(config,{at:{file:'cases.ts',...position(lineStarts(source),ordinaryOffset)},properties:false});
 assert.deepEqual([ordinary.kind,ordinary.summary,ordinary.errorType],['value','number',false]);
 const member=await compilerHookOracle(config,request('parameterModifier'));
 assert.equal(member.summary,'string');
 assert.equal(member.errorType,false);
 await assert.rejects(typeOracle(config,request('parameterName')),/Unsupported public type role: parameter-property symbol pair/);
 await assert.rejects(typeOracle(config,request('parameterModifier')),/Unsupported public type role: parameter-property symbol pair/);
});

test('Q4 declared and value forms preserve the selected question and root error flag',async()=>{
 for(const [marker,kind,summary] of [
  ['classDeclaration','declared','Local'],['classValue','expression','typeof Local'],
  ['enumDeclaration','declared','LocalState'],['enumMember','value','LocalState.Ready'],
  ['enumValue','expression','typeof LocalState'],['activeNamespace','value','typeof Active'],
  ['internalEquals','value','typeof Active'],['shorthand','value','"fixed"'],
  ['getter','value','number'],['setter','value','number'],
 ]) {
  const answer=await typeOracle(config,request(marker));
  assert.deepEqual([answer.kind,answer.summary,answer.errorType],[kind,summary,false],marker);
  assert.deepEqual(answer.prelude,[],marker);
  assert.deepEqual(answer.diagnostics,[],marker);
 }
 const failure=await typeOracle(config,request('unresolvedType'));
 assert.deepEqual([failure.kind,failure.summary,failure.errorType],['type','Missing',true]);
 const exportLiteral=await typeOracle(config,request('defaultExpression'));
 assert.deepEqual([exportLiteral.kind,exportLiteral.summary,exportLiteral.errorType],['expression','any',true]);
});

test('Q4 import and export aliases use target meaning and both renamed sides',async()=>{
 for(const [left,right,kind,summary] of [
  ['classImportLeft','classImportRight','value','typeof Widget'],
  ['typeClassLeft','typeClassRight','declared','Widget'],
  ['specifierTypeLeft','specifierTypeRight','declared','Widget'],
  ['interfaceLeft','interfaceRight','declared','Shape'],
  ['typeFunctionLeft','typeFunctionRight','value','() => 17'],
  ['typeVariableLeft','typeVariableRight','value','"fixed"'],
  ['exportLeft','exportRight','value','typeof Widget'],
  ['typeExportLeft','typeExportRight','declared','Widget'],
  ['specifierExportLeft','specifierExportRight','declared','Widget'],
  ['functionExportLeft','functionExportRight','value','() => 17'],
  ['variableExportLeft','variableExportRight','value','"fixed"'],
 ]) {
  const control=await typeOracle(config,{...request(right),question:kind});
  assert.deepEqual([control.summary,control.errorType],[summary,false],right);
  let followedOrigins;
  for(const marker of [left,right]) {
   const answer=await typeOracle(config,request(marker));
   assert.deepEqual([answer.kind,answer.summary,answer.errorType],[kind,summary,false],marker);
   assert.ok(answer.origins.length>0,marker);
   assert.ok(answer.origins.every(origin=>origin.location.file==='exports.ts'),marker);
   if(followedOrigins) assert.deepEqual(answer.origins,followedOrigins,marker);
   followedOrigins=answer.origins;
   assert.deepEqual(answer.prelude,[],marker);
  }
 }
 for(const [marker,kind,summary] of [['defaultOrdinary','value','typeof DefaultClass'],['defaultType','declared','DefaultClass'],['importEquals','value','typeof EqualClass']]) {
  const answer=await typeOracle(config,request(marker));
  assert.deepEqual([answer.kind,answer.summary,answer.errorType],[kind,summary,false],marker);
 }
});

test('Q4 external module namespace aliases ask value even when the module only exports types',async()=>{
 for(const marker of ['namespaceImport','typeNamespaceImport']) {
  const control=await typeOracle(config,{...request(marker),question:'value'});
  assert.equal(control.errorType,false,marker);
  const answer=await typeOracle(config,request(marker));
  assert.deepEqual([answer.kind,answer.summary,answer.errorType],['value',control.summary,false],marker);
 }
 const typesOnly=await typeOracle(config,{at:at('typeNamespaceImport')});
 assert.deepEqual(typesOnly.properties,[]);
});

test('Q4 type reference substitution and type-query value operands stay distinct',async()=>{
 const reference=await typeOracle(config,{at:at('typeReference')});
 assert.equal(reference.kind,'type');
 assert.equal(reference.errorType,false);
 assert.equal(reference.properties.find(property=>property.name==='value')?.summary,'string');
 for(const [marker,summary] of [['typeQuery','typeof Local'],['qualifiedTypeQuery','1']]) {
  const control=await typeOracle(config,{...request(marker),question:'value'});
  assert.deepEqual([control.summary,control.errorType],[summary,false],marker);
  const answer=await typeOracle(config,request(marker));
  assert.deepEqual([answer.kind,answer.summary,answer.errorType],['value',summary,false],marker);
 }
 await assert.rejects(typeOracle(config,request('earlierNamespace')),/UnsupportedTarget: earlier namespace segment/);
});

for(const [marker,summary,prelude] of [
  ['elementKey','"n"',[]],['elementClose','number',[]],
  ['callee','(value: number) => "result"',['CallExpression']],
  ['callClose','"result"',['CallExpression']],['tagClose','"tagged"',['TaggedTemplateExpression']],
  ['newKeyword','Local',['NewExpression']],
  ['equalsExpression','(value: number) => "result"',['CallExpression']],
 ]) test(`Q4 expression anchor ${marker} keeps its own question and required prelude`,async()=>{
  const answer=await typeOracle(config,request(marker));
  assert.deepEqual([answer.kind,answer.summary,answer.errorType],['expression',summary,false],marker);
  assert.deepEqual(answer.prelude.map(call=>call.kind),prelude,marker);
});

test('Q4 unsupported catalog and JSON positions remain explicit target failures',async()=>{
 for(const marker of ['passiveNamespace','constructorKeyword','callSignature','constructSignature','indexSignature','moduleBlock'])
  await assert.rejects(typeOracle(config,request(marker)),/UnsupportedTarget/,marker);
 await assert.rejects(typeOracle(config,{at:{file:'keys.json',line:1,column:2}}),/UnsupportedTarget: Q4 positions inside JSON/);
});

test('Q4 nested enclosing calls run outermost first in fresh baseline checkers',async()=>{
 const historyRequest={at:at('historyParameter','history.ts'),properties:false};
 const baseline=await typeOracle(config,historyRequest);
 assert.deepEqual([baseline.kind,baseline.summary,baseline.errorType],['value','string',false]);
 assert.deepEqual(baseline.prelude.map(call=>call.kind),['CallExpression','CallExpression']);
 assert.ok(baseline.prelude[0].location.start.column<baseline.prelude[1].location.start.column);
 const cold=await typeOracle(config,{...historyRequest,history:'cold'});
 assert.deepEqual([cold.kind,cold.summary,cold.errorType],['value','any',false]);
 assert.deepEqual(cold.prelude,[]);
 const diagnostics=await typeOracle(config,{...historyRequest,history:'diagnostics'});
 assert.deepEqual([diagnostics.kind,diagnostics.summary,diagnostics.errorType],['value','any',false]);
 assert.deepEqual(diagnostics.prelude,[]);
 assert.equal((await typeOracle(config,{at:at('historyResult','history.ts'),properties:false})).summary,'"second"');
 const coldAgain=await typeOracle(config,{...historyRequest,history:'cold'});
 assert.deepEqual([coldAgain.summary,coldAgain.errorType,coldAgain.prelude],['any',false,[]]);
});

test('Q4 positions preserve UTF-16 and exact trivia boundaries for compiler line breaks',async t=>{
 const root=fs.mkdtempSync(scratchPath('type-selection-positions-'));
 t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
 const config=path.join(root,'tsconfig.json');
 fs.writeFileSync(config,JSON.stringify({compilerOptions:{strict:true,noEmit:true,noLib:true},files:['cases.ts']}));
 for(const breakText of ['\n','\r','\r\n','\u2028','\u2029']) for(const whitespace of [' ','\t','\u00a0']) for(const bom of ['', '\ufeff']) {
  const marker='/*#query*/';
  const source=`${bom}// leading 💡${breakText}export class${whitespace}/*💡*/${marker}A {}${breakText}`;
  fs.writeFileSync(path.join(root,'cases.ts'),source);
  const starts=lineStarts(source),offset=source.indexOf(marker)+marker.length;
  const at={file:'cases.ts',...position(starts,offset)};
  assert.deepEqual([at.line,at.column],[2,source.slice(source.indexOf('export class'),offset).length+1]);
  const answer=await typeOracle(config,{at,properties:false});
  assert.deepEqual([answer.kind,answer.summary,answer.errorType],['declared','A',false]);
  assert.deepEqual(answer.location.start,{line:at.line,column:at.column});
  for(const triviaOffset of [source.indexOf(whitespace,source.indexOf('export class')+'export class'.length),source.indexOf('/*💡*/')])
   await assert.rejects(typeOracle(config,{at:{file:'cases.ts',...position(starts,triviaOffset)}}),/UnsupportedTarget: whitespace and comments have no type anchor/);
 }
});

test('Q4 same-checker type control exposes actual question and selected member role',async()=>{
 const member=await compilerHookOracle(config,request('parameterModifier'));
 assert.deepEqual([member.kind,member.symbolFlags,member.symbolMode],['value',4,'parameterProperty']);
 const parameter=await compilerHookOracle(config,request('parameterName'));
 assert.deepEqual([parameter.kind,parameter.symbolFlags,parameter.symbolMode],['value',1,'parameter']);
 assert.deepEqual([parameter.summary,parameter.errorType],['string',false]);
});

test('Q4 same-checker type control retains unresolved root invalidity',async()=>{
 const answer=await compilerHookOracle(config,request('unresolvedType'));
 assert.deepEqual([answer.kind,answer.summary,answer.errorType],['type','Missing',true]);
});

test('Q4 producing modifier variants select exact member and lexical parameter roles',async()=>{
 const root=fileURLToPath(new URL('../fixtures/types/parameter-properties/',import.meta.url));
 const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
 const config=path.join(root,'tsconfig.json');
 const request=marker=>{
  const text=`/*#${marker}*/`,offset=source.indexOf(text);
  assert.notEqual(offset,-1,marker);
  return {at:{file:'cases.ts',...position(lineStarts(source),offset+text.length)},properties:false};
 };
 for(const marker of ['public','private','protected','readonly','override','mixed']) {
  const member=await compilerHookOracle(config,request(marker));
  assert.deepEqual([member.kind,member.symbolFlags,member.symbolMode,member.summary,member.errorType],['value',4,'parameterProperty','number',false],marker);
  const parameter=await compilerHookOracle(config,request(`${marker}Name`));
  assert.deepEqual([parameter.kind,parameter.symbolFlags,parameter.symbolMode,parameter.summary,parameter.errorType],['value',1,'parameter','number',false],marker);
  await assert.rejects(typeOracle(config,request(marker)),/symbol pair requires the compiler hook/);
  await assert.rejects(typeOracle(config,request(`${marker}Name`)),/symbol pair requires the compiler hook/);
 }
 await assert.rejects(typeOracle(config,request('secondary')),/UnsupportedTarget/);
 await assert.rejects(compilerHookOracle(config,request('secondary')),/UnsupportedTarget/);
});

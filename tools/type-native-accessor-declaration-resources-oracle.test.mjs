import {assertFixtureCapture} from './type-oracle-fixture.mjs';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {loadSemanticApi,lineStarts,position} from './semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-accessor-declaration-resources');
test('native accessor declaration lists support independent read and write roots',async()=>{
 const native=await loadSemanticApi();assert.equal(native.version,'7.0.2');
 const rows=[];
 for(const variant of JSON.parse(fs.readFileSync(path.join(root,'variants.json'),'utf8'))) {
  const config=path.join(root,variant.config),source=fs.readFileSync(path.join(root,variant.file),'utf8');
  // A separate fresh checker observes raw list size, without warming either
  // independently captured final query's evaluator state.
  const api=new native.API({tsserverPath:native.binary,cwd:root});let snapshot;let declarations;
  try {
   snapshot=api.updateSnapshot({openProject:config});const project=snapshot.getProject(config),program=project.program;
   for(const diagnostics of [program.getConfigFileParsingDiagnostics(),program.getProgramDiagnostics(),program.getSyntacticDiagnostics(),program.getBindDiagnostics()])assert.deepEqual(diagnostics,[]);
   const parsed=program.getSourceFile(path.join(root,variant.file));
   const item=parsed.statements.flatMap(s=>s.declarationList?.declarations??[]).find(d=>d.name?.text==='item');assert.ok(item);
   const type=project.checker.getTypeOfSymbol(project.checker.getSymbolAtLocation(item.name));
   const properties=project.checker.getPropertiesOfType(type);assert.equal(properties.length,variant.width);
   declarations=properties.map((property,index)=>{
    assert.equal(property.name,`p${index}`);assert.equal(property.declarations.length,2);
    return {name:property.name,origins:property.declarations.map(handle=>{
     const own=handle.resolve(project);assert.ok(own);
     return {kind:native.SyntaxKind[own.kind],location:{file:variant.file,start:position(lineStarts(source),own.name.getStart(parsed)),end:position(lineStarts(source),own.name.end)}};
    })};
   });
   for(const p of declarations)assert.deepEqual(p.origins.map(o=>o.kind),['GetAccessor','SetAccessor']);
  } finally {try {snapshot?.dispose();} finally {api.close();}}
  for(const mode of ['read','write']) {
   const marker=`/*#${mode}*/`;const at={file:variant.file,...position(lineStarts(source),source.indexOf(marker)+marker.length)};
   const answer=await compilerHookOracle(config,{operation:'type',at});
   rows.push({variant,mode,source,at,declarations,answer});
   fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/native-accessor-declaration-resources.json',JSON.stringify(rows,null,2)+'\n');
   assert.equal(answer.compiler,'7.0.2');assert.equal(answer.checkerCount,1);assert.equal(answer.summary,'number[]');assert.equal(answer.errorType,false);
   assert.deepEqual(answer.diagnosticCodes,[]);assert.deepEqual(answer.demandFailures,[]);
  }
 }
 assert.equal(rows.length,32);
 const golden=path.join(root,'expected.json');if(fs.existsSync(golden))assertFixtureCapture(rows,JSON.parse(fs.readFileSync(golden,'utf8')),root);
});

test('contextual getter return annotations acquire paired setter declarations',async()=>{
 const rows=[],source=fs.readFileSync(path.join(root,'signatures.ts'),'utf8');
 for(const profile of ['strict','nocheck','loose','loose-nocheck']) for(const [key,expected] of [['literal','number'],['symbol','string'],['plain','boolean']]) {
  const config=`signatures-${profile}.json`,marker=`/*#${key}*/`;
  const at={file:'signatures.ts',...position(lineStarts(source),source.indexOf(marker)+marker.length)};
  const answer=await compilerHookOracle(path.join(root,config),{operation:'type',at});
  rows.push({profile,key,config,configuration:JSON.parse(fs.readFileSync(path.join(root,config),'utf8')),source,at,answer});
  fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/native-accessor-signature-resources.json',JSON.stringify(rows,null,2)+'\n');
  assert.equal(answer.compiler,'7.0.2');assert.equal(answer.checkerCount,1);assert.equal(answer.summary,expected);assert.equal(answer.errorType,false);
  assert.deepEqual(answer.diagnosticCodes,[]);assert.deepEqual(answer.demandFailures,[]);
 }
 assert.equal(rows.length,12);
 const golden=path.join(root,'expected-signatures.json');if(fs.existsSync(golden))assertFixtureCapture(rows,JSON.parse(fs.readFileSync(golden,'utf8')),root);
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {loadSemanticApi,lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-keyof/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected={"shape": "keyof Shape", "alias": "keyof Alias", "literal": "\"a\" | \"z\" | 4", "classKeys": "keyof Example", "stringIndex": "string | number", "numberIndex": "number | \"named\"", "brokenIndex": "string | number", "union": "\"common\"", "intersection": "\"left\" | \"right\"", "anyKeys": "string | number | symbol", "neverKeys": "string | number | symbol", "unknownKeys": "never", "objectKeys": "never", "emptyKeys": "never", "stringKeys": "\"charAt\" | \"length\"", "numberKeys": "\"toFixed\"", "booleanKeys": "\"valueOf\"", "nullKeys": "never", "undefinedKeys": "never", "tupleKeys": "keyof [first: number, second?: string | undefined]", "instantiated": "\"a\" | \"b\"", "generic": "keyof T", "genericIntersection": "keyof (T & {})", "genericUnion": "never", "invalid": "string | number | symbol"};
const names=['shape','alias','literal','classKeys','stringIndex','numberIndex','brokenIndex','union','intersection','anyKeys','neverKeys','unknownKeys','objectKeys','emptyKeys','stringKeys','numberKeys','booleanKeys','nullKeys','undefinedKeys','tupleKeys','instantiated','generic','genericIntersection','genericUnion','invalid'];
for (const config of ['tsconfig.json','loose.json','exact.json']) {
 test(`native keyof collection (${config})`,async()=>{
  const evidence=[];
  for (const name of names) {
   const tag=`/*#${name}*/`;const tagged=source.indexOf(tag);
   const offset=tagged<0?source.indexOf(`const ${name}:`)+6:tagged+tag.length;
   const answer=await typeOracle(path.join(root,config),{at:{file:'cases.ts',...position(lineStarts(source),offset)},properties:false});
   evidence.push({name,summary:answer.summary,errorType:answer.errorType,kind:answer.kind});
   fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync(`.cc/temp/native-keyof-${config}`,JSON.stringify(evidence,null,2));
   assert.equal(answer.compiler,'7.0.2');
   const summary=name==='tupleKeys'&&config!=='tsconfig.json'?'keyof [first: number, second?: string]':expected[name];
   assert.equal(answer.summary,summary,`${config}/${name}`);
   assert.equal(answer.errorType,false,`${config}/${name}`);
  }
 });
}

// Public follow-up observations inspect the actual returned union and index
// payloads. The native getIndexType/getIndexInfosOfIndexSymbol source proves
// their acquisition order; printed keyof summaries alone conceal recovery.
test('native keyof preserves actual key constituents and consumed recovery',async()=>{
 const native=await loadSemanticApi();
 for (const profile of ['tsconfig.json','loose.json','exact.json']) {
  const config=path.join(root,profile);const api=new native.API({tsserverPath:native.binary,cwd:root});let snapshot;
  try {
   snapshot=api.updateSnapshot({openProject:config});const project=snapshot.getProject(config);const checker=project.checker;
   const source=project.program.getSourceFile(path.join(root,'cases.ts'));
   const variable=name=>source.statements.flatMap(statement=>statement.declarationList?.declarations??[]).find(declaration=>declaration.name.text===name);
   for (const [name,expected] of [['shape',['"a"','"broken"','"z"','2']],['alias',['"first"','"later"']],['classKeys',['"method"','"visible"','3']]]) {
    const declaration=variable(name);const type=checker.getTypeOfSymbol(checker.getSymbolAtLocation(declaration.name));
    assert.deepEqual(type.getTypes().map(child=>checker.typeToString(child)),expected,`${profile}/${name}`);
   }
   const broken=variable('brokenIndex');checker.getTypeOfSymbol(checker.getSymbolAtLocation(broken.name));
   const infos=checker.getIndexInfosOfType(checker.getTypeAtLocation(broken.type.type));
   assert.equal(infos.length,1);assert.equal(infos[0].valueType.isErrorType(),true,profile);
   const invalid=variable('invalid');checker.getTypeOfSymbol(checker.getSymbolAtLocation(invalid.name));
   assert.equal(checker.getTypeAtLocation(invalid.type.type).isErrorType(),true,profile);
  } finally {try {snapshot?.dispose();} finally {api.close();}}
 }
});

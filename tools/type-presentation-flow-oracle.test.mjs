import assert from 'node:assert/strict';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/presentation-flow/',import.meta.url));
test('native lazy member and index dependencies consume a getter with a changed flow read',async()=>{
 const native=await loadSemanticApi();
 assert.equal(native.version,'7.0.2');
 for(const configName of ['tsconfig.json','loose.json','exact.json']){
  const config=path.join(root,configName),api=new native.API({tsserverPath:native.binary,cwd:root});
  let snapshot;
  try{
   snapshot=api.updateSnapshot({openProject:config});
   const project=snapshot.getProject(config),checker=project.checker;
   const source=project.program.getSourceFile(path.join(root,'cases.ts'));
   const type=(name)=>{const token=native.getTokenAtPosition(source,source.text.indexOf(`const ${name}:`)+6);return checker.getTypeOfSymbol(checker.getSymbolAtLocation(token));};
   const array=type('array'),host=type('host');
   assert.equal(checker.typeToString(array),'number[]');
   assert.equal(checker.typeToString(host),'Host');
   const arrayIndex=checker.getIndexInfosOfType(array)[0].valueType;
   const direct=checker.getTypeOfSymbol(checker.getPropertyOfType(host,'direct'));
   const nested=checker.getTypeOfSymbol(checker.getPropertyOfType(host,'nested'));
   const nestedIndex=checker.getIndexInfosOfType(nested)[0].valueType;
   const shared=checker.getTypeOfSymbol(checker.getPropertyOfType(type('shared'),'value'));
   const independent=checker.getTypeOfSymbol(checker.getPropertyOfType(type('independent'),'value'));
   for(const value of [arrayIndex,direct,nestedIndex,shared,independent,type('later')]){
    assert.equal(value.isErrorType(),false);
    assert.equal(checker.typeToString(value),'string | 0');
   }
  }finally{try{snapshot?.dispose();}finally{api.close();}}
 }
});

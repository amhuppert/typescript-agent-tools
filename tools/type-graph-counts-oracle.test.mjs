import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';

test('native wide tuple establishes one shared numeric type for every argument',async()=>{
 const native=await loadSemanticApi();
 assert.equal(native.version,'7.0.2');
 const directory=fs.mkdtempSync(path.join(os.tmpdir(),'tsparse-graph-counts-'));
 try {
  for(const count of [3000,5000]) {
  const elements=Array(count).fill('number').join(',');
  const text=`type Wide = [${elements}];\ninterface Array<T> { length: number; [index: number]: T }\ninterface ReadonlyArray<T> { readonly length: number; readonly [index: number]: T }\ndeclare const value: Wide;\n`;
  fs.writeFileSync(path.join(directory,'cases.ts'),text);
  for(const [strict,exactOptionalPropertyTypes] of [[false,false],[true,false],[true,true]]){
   const config=path.join(directory,'tsconfig.json');
   fs.writeFileSync(config,JSON.stringify({compilerOptions:{strict,exactOptionalPropertyTypes,noLib:true},files:['cases.ts']}));
   const api=new native.API({tsserverPath:native.binary,cwd:directory});
   let snapshot;
   try{
    snapshot=api.updateSnapshot({openProject:config});
    const project=snapshot.getProject(config),checker=project.checker;
    const file=project.program.getSourceFile(path.join(directory,'cases.ts'));
    const token=native.getTokenAtPosition(file,text.lastIndexOf('value:'));
    const type=checker.getTypeOfSymbol(checker.getSymbolAtLocation(token));
    assert.equal(checker.typeToString(type),'Wide');
    assert.equal(type.isErrorType(),false);
    assert.equal(checker.isTupleType(type),true);
    const args=checker.getTypeArguments(type);
    assert.equal(args.length,count);
    assert.equal(checker.typeToString(args[0]),'number');
    assert.ok(args.every(arg=>arg===args[0]));
   }finally{try{snapshot?.dispose();}finally{api.close();}}
  }
  }
 }finally{fs.rmSync(directory,{recursive:true,force:true});}
});

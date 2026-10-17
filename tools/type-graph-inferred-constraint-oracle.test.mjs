import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';

const root=fileURLToPath(new URL('../fixtures/types/graph-inferred-constraint/',import.meta.url));
test('native inferred constraint consumes a getter after its parameter header',async()=>{
 const native=await loadSemanticApi();
 assert.equal(native.version,'7.0.2');
 const rows=[];
 for(const config of ['tsconfig.json','loose.json','exact.json']) {
  const file=path.join(root,config), api=new native.API({tsserverPath:native.binary,cwd:root});
  let snapshot;
  try {
   snapshot=api.updateSnapshot({openProject:file});
   const project=snapshot.getProject(file),checker=project.checker;
   const source=project.program.getSourceFile(path.join(root,'cases.ts'));
   const token=native.getTokenAtPosition(source,source.text.indexOf('/*#query*/')+'/*#query*/'.length);
   const describe=type=>({summary:checker.typeToString(type),errorType:type.isErrorType()});
   const type=checker.getDeclaredTypeOfSymbol(checker.getSymbolAtLocation(token));
   const before=describe(type),constraint=checker.getConstraintOfTypeParameter(type);
   assert.ok(constraint);
   rows.push({config,before,constraint:describe(constraint),after:describe(type)});
  } finally {try{snapshot?.dispose();}finally{api.close();}}
 }
 fs.writeFileSync('.cc/temp/graph-inferred-constraint-native.json',JSON.stringify(rows,null,2));
 for(const row of rows){
  assert.deepEqual(row.before,{summary:'U',errorType:false});
  assert.deepEqual(row.constraint,{summary:'string | 0',errorType:false});
  assert.deepEqual(row.after,row.before);
 }
});

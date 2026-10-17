import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-module-wrappers/',import.meta.url));
test('native module wrappers retain named, optional and default properties',async()=>{
 const rows=[];
 for(const config of ['tsconfig','loose','exact']) {
  const file='cases.ts',source=fs.readFileSync(path.join(root,file),'utf8');
  for(const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
   const answer=await compilerHookOracle(path.join(root,config+'.json'),{operation:'type',at:{file,...position(lineStarts(source),marker.index+marker[0].length)}});
   const row={config,name:marker[1],summary:answer.summary,errorType:answer.errorType,before:answer.beforeSummary.diagnosticCodes,after:answer.diagnosticCodes,failures:answer.demandFailures};
   rows.push(row);
   assert.equal(answer.compiler,'7.0.2');
   const summary={callableLabel:'"callable"',callableDefault:'typeof run',objectLabel:'"object"',objectOptional:config==='loose'?'number':'number | undefined',objectFixed:'1',realDefault:'() => "default"',realLabel:'"real"',callableNamespace:'typeof run',objectNamespace:`{ label: "object"; optional?: ${config==='tsconfig'?'number | undefined':'number'}; readonly fixed: 1; }`,realNamespace:`typeof import(${JSON.stringify(path.join(root,'default'))})`}[row.name];
   assert.equal(row.summary,summary,`${config}/${row.name}`);
   assert.equal(row.errorType,false,`${config}/${row.name}`);
   assert.deepEqual(row.before,[],`${config}/${row.name}/before`);
   assert.deepEqual(row.after,[],`${config}/${row.name}/after`);
   assert.deepEqual(row.failures,[],`${config}/${row.name}/failures`);
  }
 }
 fs.writeFileSync('.cc/temp/types-module-wrappers-native.json',JSON.stringify(rows,null,2));
 assert.equal(rows.length,30);
});

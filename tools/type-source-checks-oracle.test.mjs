import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-type-checks/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected={valid:['1',[]],rejected:['string',[]],hidden:['number',[]],later:['number',[]],skipped:['number',[]],argument:['number',[2304]],query:['number',[]],satisfied:['1',[]],satisfiesHidden:['1',[2304]],named:['[value: 1]',[]],indexed:['1',[]],angle:['1',[]],assertionOrder:['number',[]],satisfiesOrder:['any',[2304,2304]],checkedRejected:['string',[2344]],checkedHidden:['number',[2304]],checkedLater:['number',[2304,2344]],checkedSkipped:['number',[2344]],checkedAssertionOrder:['number',[2304,2304]],checkedAngle:['number',[2304]],checkedFunction:['(value: number) => string',[]],checkedMissingFunction:['(value: MissingParameter) => number',[2304]],checkedMissingConstructor:['new (value: MissingConstructorParameter) => object',[2304]]};
test('type-node source checks retain assertion and satisfies dependencies',async t=>{
 const rows=[];
 for(const config of ['tsconfig.json','loose.json','exact.json']) {
  for(const [name,[summary,codes]] of Object.entries(expected)) {
   const tag=`/*#${name}*/`;
   const at={file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)};
   const answer=await typeOracle(path.join(root,config),{at,properties:false});
   const hook=await compilerHookOracle(path.join(root,config),{operation:'type',at});
   rows.push({config,name,summary:answer.summary,errorType:answer.errorType,codes:hook.beforeSummary.diagnosticCodes,failures:hook.beforeSummary.demandFailures});
   assert.equal(answer.compiler,'7.0.2');
   assert.equal(answer.summary,hook.summary,`${config}/${name}/hook`);
  }
 }
 t.diagnostic(JSON.stringify(rows));
 for(const row of rows) {
  const [summary,codes]=expected[row.name];
  assert.equal(row.summary,summary,`${row.config}/${row.name}`);
  assert.equal(row.errorType,row.name==='satisfiesOrder',`${row.config}/${row.name}`);
  assert.deepEqual(row.codes,codes,`${row.config}/${row.name}/dependencies`);
  assert.deepEqual(row.failures,[],`${row.config}/${row.name}`);
 }
});

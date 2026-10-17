import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-recovery-observation-resources');
test('native unknown-signature acquisition counts for repeated identifier calls',async t=>{
 const rows=[];
 for(const width of [16,64]) {
  const file=`a-${width}.ts`,source=fs.readFileSync(path.join(root,file),'utf8'),marker='/*#chosen*/';
  const at={file,...position(lineStarts(source),source.indexOf(marker)+marker.length)};
  const answer=await compilerHookOracle(path.join(root,`ascii-${width}-strict.json`),{operation:'type',at,inspectUnknownSignatureReturns:true,inspectDiagnosticCache:true,inspectExpressionAcquisitions:true});
  rows.push({width,source,at,answer});
  fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/recovery-observation-identifier-trace.json',JSON.stringify(rows,null,2)+'\n');
  assert.equal(answer.compiler,'7.0.2');assert.equal(answer.summary,'17');assert.equal(answer.errorType,false);
  assert.deepEqual(answer.diagnosticCodes,[]);assert.deepEqual(answer.demandFailures,[]);
  assert.equal(answer.beforeSummary.observations.unknownSignatureReturn.errorType,true);
  t.diagnostic(JSON.stringify({width,unknownSignatureReturns:answer.beforeSummary.counters.unknownSignatureReturns,acquisitions:answer.beforeSummary.displayAcquisitions.length}));
 }
});

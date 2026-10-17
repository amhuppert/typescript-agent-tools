import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle,typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-template-indexes/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected={"primitiveString": "\"string\"", "primitiveNumber": "\"number\"", "primitiveSymbol": "\"symbol\"", "patternNumber": "\"pattern\"", "patternBigInt": "\"big\"", "patternString": "\"str\"", "patternBad": "any", "mappingUpper": "\"upper\"", "mappingBad": "any", "mappingNested": "\"nested\"", "mappingTemplate": "\"mapped-template\"", "brandString": "\"brand-string\"", "brandNumber": "\"brand-number\"", "brandPattern": "\"brand-pattern\"", "intersectionRejected": "never", "literalRejected": "never", "anyRejected": "never", "booleanPatternRejected": "never", "unionPrimitive": "\"all\"", "unionPatterns": "\"union\"", "mixedUnionKey": "\"valid-part\"", "overlapFallback": "\"pattern\"", "overlapPatterns": "never", "duplicate": "\"first\"", "duplicateUnion": "\"first\"", "readonlyPattern": "\"readonly\"", "genericRejected": "never", "genericPatternRejected": "never", "genericMappingRejected": "never", "duplicateFailure": "\"first\"", "invalidKeyFailure": "any", "valueBeforeKey": "any", "missingKey": "any", "patternExpression": "\"pattern\""};
const errors=new Set(['patternBad','mappingBad','invalidKeyFailure','valueBeforeKey','missingKey']);
const dependencies={duplicateFailure:[2304],invalidKeyFailure:[2304,2537],valueBeforeKey:[2304,2304,2537],missingKey:[2304,2537]};
test('native index inventories retain pattern, mapping, nongeneric intersection and ordered dependencies',async t=>{
 const rows=[];
 for(const config of ['tsconfig.json','loose.json','exact.json','unchecked.json']) for(const marker of Object.keys(expected)) {
  const tag=`/*#${marker}*/`;
  const at={file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)};
  const answer=await typeOracle(path.join(root,config),{at});
  let codes;
  if(marker in dependencies){const hook=await compilerHookOracle(path.join(root,config),{operation:'type',at});assert.equal(answer.summary,hook.summary);codes=hook.beforeSummary.diagnosticCodes;}
  rows.push({config,marker,summary:answer.summary,error:answer.errorType,codes});
 }
 fs.mkdirSync('.cc/temp',{recursive:true});fs.writeFileSync('.cc/temp/types-template-indexes-observations.json',JSON.stringify(rows,null,2));
 t.diagnostic(JSON.stringify(rows));
 for(const row of rows){
  const summary=row.marker==='patternExpression'&&row.config==='unchecked.json'?'"pattern" | undefined':expected[row.marker];
  assert.equal(row.summary,summary,`${row.config}/${row.marker}`);
  assert.equal(row.error,errors.has(row.marker),`${row.config}/${row.marker}`);
  if(row.codes) assert.deepEqual(row.codes,dependencies[row.marker],`${row.config}/${row.marker}/dependencies`);
 }
});

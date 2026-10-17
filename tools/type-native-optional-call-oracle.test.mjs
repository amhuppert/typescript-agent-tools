import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle, compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-optional-calls/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const markers=[...source.matchAll(/\/\*#([^*]+)\*\//g)].map(match=>match[1]);
function at(marker) { const tag=`/*#${marker}*/`; return {file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)}; }
const base = {
 plain: '{ value: "plain"; }', plainProperty: '"plain"',
 inferred: '{ value: number; }', explicit: '{ value: string; }', inferredProperty: 'number',
 overload: '"number"', receiver: '{ value: number; }', receiverProperty: 'number',
 both: '{ value: number; }', bothProperty: 'number', required: '1', any: 'any', anyBad: 'any',
 missingReturn: 'any', nested: '"nested"', grouped: '"nested"',
 explicitThis: '"receiver"', optionalThis: '"optionalReceiver"', nullableReturn: 'string',
};
const unchanged = new Set(['required', 'any', 'anyBad', 'missingReturn', 'grouped']);
assert.deepEqual(markers, Object.keys(base));
for(const config of ['tsconfig.json','exact.json','loose.json']) {
 test(`native optional calls (${config})`,async()=>{
  for(const marker of markers) {
   const loose = config === 'loose.json';
   let summary = base[marker];
   if (!loose && !unchanged.has(marker)) summary += ' | undefined';
   if (loose && marker === 'missingReturn') summary = 'MissingReturn';
   const errorType = marker === 'missingReturn';
   const codes = marker === 'anyBad' || marker === 'missingReturn' ? [2304]
      : marker === 'grouped' && !loose ? [2532, 2722] : [];
   const answer=await typeOracle(path.join(root,config),{at:at(marker),properties:false});
   assert.equal(answer.compiler,'7.0.2');
   assert.equal(answer.kind,'expression', marker);
   assert.deepEqual([answer.summary, answer.errorType], [summary, errorType], marker);
   const hook=await compilerHookOracle(path.join(root,config),{operation:'type',at:at(marker)});
   assert.deepEqual([hook.summary, hook.errorType], [summary, errorType], marker);
   assert.deepEqual(hook.beforeSummary.diagnosticCodes, codes, marker);
   assert.deepEqual(hook.beforeSummary.demandFailures, [], marker);
  }
 });
}

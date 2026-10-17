import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle, compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-construct/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const markers=[...source.matchAll(/\/\*#([^*]+)\*\//g)].map(match=>match[1]);
function at(marker) { const tag=`/*#${marker}*/`; return {file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)}; }
const expected = {
 public: ['Box<number>', false, []], explicit: ['Box<string>', false, []],
 newable: ['{ readonly tag: "newable"; }', false, []],
 any: ['any', false, []], anyBad: ['any', false, [2304]],
 callOnly: ['any', false, [7009]], callMissing: ['any', false, [7009]],
 privateInside: ['Private', false, []], privateNested: ['Private', false, []],
 privateOutside: ['Private', false, [2673]],
 protectedInside: ['Protected', false, []], protectedGrandchild: ['Protected', false, []],
 protectedOutside: ['Protected', false, [2674]], inheritedOutside: ['Derived', false, [2674]],
 abstract: ['Abstract', false, [2511]], abstractSignature: ['{ value: number; }', false, [2511]],
 voidParen: ['{ value: "void"; }', false, []], voidBare: ['{ value: "void"; }', false, []],
 bare: ['{ value: "empty"; }', false, []], argumentFailure: ['Box<any>', false, [2304]],
 abstractArgument: ['Abstract', false, [2511, 2304]], genericAbstract: ['any', true, [2511]],
 checkedPrivate: ['any', true, [2673]], checkedAbstract: ['any', true, [2511]],
 notConstructor: ['any', true, [2351]], receiverCall: ['any', false, [7009]],
 receiverMissing: ['any', false, [2304, 7009]], unionAbstract: ['any', true, [2511]],
};
const looseCodes = { callOnly: [2350], callMissing: [2304, 2350], receiverCall: [], receiverMissing: [2304] };
const unknown = new Set(['privateOutside', 'protectedOutside', 'inheritedOutside', 'abstract', 'abstractSignature', 'abstractArgument', 'genericAbstract', 'checkedPrivate', 'checkedAbstract', 'notConstructor', 'unionAbstract']);
assert.deepEqual(markers, Object.keys(expected));
for(const config of ['tsconfig.json','exact.json','loose.json']) {
 test(`native checked constructors (${config})`,async()=>{
  for(const marker of markers) {
   const [summary, errorType, strictCodes] = expected[marker];
   const codes = config === 'loose.json' ? (looseCodes[marker] ?? strictCodes) : strictCodes;
   const answer=await typeOracle(path.join(root,config),{at:at(marker),properties:false});
   assert.equal(answer.compiler,'7.0.2');
   assert.deepEqual([answer.summary, answer.errorType], [summary, errorType], marker);
   const hook=await compilerHookOracle(path.join(root,config),{operation:'type',at:at(marker)});
   assert.deepEqual([hook.summary, hook.errorType], [summary, errorType], marker);
   assert.deepEqual(hook.beforeSummary.diagnosticCodes, codes, marker);
   assert.deepEqual(hook.beforeSummary.demandFailures, [], marker);
   // Observe the same checker's signature sentinel without demanding its return.
   // Quick type queries can stay healthy after the prelude picked unknownSignature.
   const resolution = Object.entries(hook.beforeSummary.observations)
     .filter(([key]) => key.startsWith('constructorResolution:')).map(([, value]) => value);
   assert.deepEqual(resolution, [{anySignature: marker === 'any' || marker === 'anyBad',
     silentNeverSignature: false, unknownSignature: unknown.has(marker)}], marker);
  }
 });
}

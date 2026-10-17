import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-instantiation-expressions/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const markers=['runtime', 'call', 'defaults', 'empty', 'rejected', 'arity', 'nongeneric', 'both', 'property', 'constructed', 'index', 'union', 'intersection', 'mixed', 'any', 'never', 'literal', 'outer', 'constraint', 'returnFailure', 'argumentFailure', 'operandFailure'];
for(const config of ['tsconfig.json','exact.json','loose.json']){
 test(`native instantiation expressions (${config})`,async()=>{
  const observed=[];
  for(const marker of markers){
   const tag=`/*#${marker}*/`;
   const answer=await typeOracle(path.join(root,config),{at:{file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)},properties:false});
   assert.equal(answer.compiler,'7.0.2');
   observed.push({marker,summary:answer.summary,errorType:answer.errorType});
  }
  fs.mkdirSync('.cc/temp',{recursive:true});
  fs.writeFileSync(`.cc/temp/native-instantiation-expressions-${config}`,JSON.stringify(observed,null,2));

  assert.deepEqual(observed,[
  {
    "marker": "runtime",
    "summary": "(value: number) => number",
    "errorType": false
  },
  {
    "marker": "call",
    "summary": "number",
    "errorType": false
  },
  {
    "marker": "defaults",
    "summary": "(first: string, second: string) => string",
    "errorType": false
  },
  {
    "marker": "empty",
    "summary": "(value: unknown) => unknown",
    "errorType": false
  },
  {
    "marker": "rejected",
    "summary": "<T extends string>(value: T) => T",
    "errorType": false
  },
  {
    "marker": "arity",
    "summary": "{}",
    "errorType": false
  },
  {
    "marker": "nongeneric",
    "summary": "{}",
    "errorType": false
  },
  {
    "marker": "both",
    "summary": "{ (value: number): number; new (value: number): { value: number; }; readonly tag: \"tag\"; }",
    "errorType": false
  },
  {
    "marker": "property",
    "summary": "\"tag\"",
    "errorType": false
  },
  {
    "marker": "constructed",
    "summary": "number",
    "errorType": false
  },
  {
    "marker": "index",
    "summary": "string",
    "errorType": false
  },
  {
    "marker": "union",
    "summary": "((value: number) => number) | ((value: number) => number[])",
    "errorType": false
  },
  {
    "marker": "intersection",
    "summary": "\"intersection\"",
    "errorType": false
  },
  {
    "marker": "mixed",
    "summary": "{ readonly tag: \"mixed\"; } | ((value: number) => number)",
    "errorType": false
  },
  {
    "marker": "any",
    "summary": "any",
    "errorType": false
  },
  {
    "marker": "never",
    "summary": "never",
    "errorType": false
  },
  {
    "marker": "literal",
    "summary": "\"literal\"",
    "errorType": false
  },
  {
    "marker": "outer",
    "summary": "(value: number) => number",
    "errorType": false
  },
  {
    "marker": "constraint",
    "summary": "(value: number) => number",
    "errorType": false
  },
  {
    "marker": "returnFailure",
    "summary": "MissingReturn",
    "errorType": true
  },
  {
    "marker": "argumentFailure",
    "summary": "(value: MissingArgument) => MissingArgument",
    "errorType": false
  },
  {
    "marker": "operandFailure",
    "summary": "any",
    "errorType": true
  }
]);
 });
}

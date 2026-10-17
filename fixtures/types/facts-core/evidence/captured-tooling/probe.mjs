import fs from 'node:fs';
import path from 'node:path';
import { compilerHookOracle } from './oracle.mjs';
const here = path.resolve('.cc/temp/types-facts');
const families = [
  ['number','number'],['zero','0'],['one','1'],['string','string'],['empty','""'],['word','"x"'],
  ['bigint','bigint'],['bigzero','0n'],['bigone','1n'],['boolean','boolean'],['false','false'],['true','true'],
  ['symbol','symbol'],['object','object'],['any','any'],['unknown','unknown'],['never','never'],['void','void'],['undefined','undefined'],['null','null'],
  ['emptyObject','{}'],['propertyObject','{ value: number }'],['callable','() => number'],['constructor','new () => number'],['indexed','{ [key: string]: number }'],
  ['numberString','number | string'],['literalUnion','0 | 1'],['nullableNumber','number | undefined | null'],['nullableUnknown','unknown | undefined | null'],
];
const answers = [];
for (const [name,annotation] of families) for (const strict of [false,true]) {
  const directory = path.join(here,`${name}-${strict}`);
  fs.mkdirSync(directory,{recursive:true});
  fs.writeFileSync(path.join(directory,'cases.ts'),`export let current: ${annotation};\n`);
  const config = path.join(directory,'tsconfig.json');
  fs.writeFileSync(config,JSON.stringify({compilerOptions:{strict,noLib:true,target:'esnext'},files:['cases.ts']})+'\n');
  const answer = await compilerHookOracle(config,{at:{file:'cases.ts',line:1,column:12},properties:false});
  answers.push({name,annotation,strict,answer});
  console.log(JSON.stringify({name,strict,summary:answer.summary,errorType:answer.errorType,facts:answer.typeFacts?.all,
    truthy:answer.typeFacts?.masks.find(row=>row.name==='truthy'),typeofNumber:answer.typeFacts?.masks.find(row=>row.name==='typeofNumber')}));
}
fs.writeFileSync(path.join(here,'answers.json'),JSON.stringify({question:'fresh selected value, then exact native facts/filter/adjusted masks in listed order; no diagnostic pass',answers},null,2)+'\n');

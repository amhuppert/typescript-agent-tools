import fs from 'node:fs';
import path from 'node:path';
import { typeOracle } from '../../tools/type-oracle.mjs';
const here = path.resolve('.cc/temp/types-facts-expressions');
fs.mkdirSync(here,{recursive:true});
const source = `export declare const literalUnion: 0 | 1;
export declare const truthyUnion: 1 | 2;
export declare const primitiveUnion: string | number;
export declare const nullableUnion: null | undefined;
export declare const mixedUnion: number | undefined;
export declare const neverValue: never;
export const negatedLiteral = !literalUnion;
export const negatedTruthy = !truthyUnion;
export const negatedPrimitive = !primitiveUnion;
export const negatedNullable = !nullableUnion;
export const negatedMixed = !mixedUnion;
export const negatedNever = !neverValue;
`;
fs.writeFileSync(path.join(here,'cases.ts'),source);
const answers = [];
for (const strict of [false,true]) {
  const config = path.join(here,`${strict}.json`);
  fs.writeFileSync(config,JSON.stringify({compilerOptions:{strict,noLib:true},files:['cases.ts']})+'\n');
  for (const name of ['negatedLiteral','negatedTruthy','negatedPrimitive','negatedNullable','negatedMixed','negatedNever']) {
    const offset = source.indexOf(name);
    const prefix = source.slice(0,offset).split('\n');
    const request = {at:{file:'cases.ts',line:prefix.length,column:prefix.at(-1).length+1},properties:false};
    const answer = await typeOracle(config,request);
    answers.push({strict,name,request,answer});
  }
}
fs.writeFileSync(path.join(here,'answers.json'),JSON.stringify({source,answers},null,2)+'\n');
console.log(JSON.stringify(answers.map(row=>({name:row.name,strict:row.strict,summary:row.answer.summary,errorType:row.answer.errorType})),null,2));

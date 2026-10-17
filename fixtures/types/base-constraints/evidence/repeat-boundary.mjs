import fs from 'node:fs';
import path from 'node:path';
import { compilerHookOracle } from './oracle.mjs';
const root = path.resolve('.cc/temp/types-base-constraints');
const answers = [];
for (const count of [5,6,7]) {
 const directory = path.join(root, `bounded-repeat-${count}`); fs.mkdirSync(directory,{recursive:true});
 const source = `interface Array<T> { length: number; }\ntype R<T, N extends number, C extends unknown[] = []> = C["length"] extends N ? "done" : T extends string ? R<{[P in keyof T]:T[P]}, N, [...C,unknown]> : number;\ninterface Host<T> { value: R<T,${count}>; }\n`;
 fs.writeFileSync(path.join(directory,'cases.ts'), source);
 const config = path.join(directory,'tsconfig.json'); fs.writeFileSync(config,JSON.stringify({compilerOptions:{strict:true,noLib:true,noEmit:true,target:'esnext'},files:['cases.ts']})+'\n');
 const line = source.split('\n')[2];
 const answer = await compilerHookOracle(config,{at:{file:'cases.ts',line:3,column:line.indexOf('R<T,')+1},properties:false});
 answers.push({count,source,answer}); console.log(JSON.stringify({count,base:answer.baseConstraint,after:answer.afterBaseConstraint}));
}
fs.writeFileSync(path.join(root,'bounded-repeat-answers.json'),JSON.stringify({sequence:'fresh selected R<T,N>; base, resolved cache sentinel; summary afterward',answers},null,2)+'\n');

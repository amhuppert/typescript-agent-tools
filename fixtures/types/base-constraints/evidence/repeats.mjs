import fs from 'node:fs';
import path from 'node:path';
import { compilerHookOracle } from './oracle.mjs';
const root = path.resolve('.cc/temp/types-base-constraints');
const families = [
 ['same','type R<T> = T extends string ? R<T> : number;'],
 ['intersection','type R<T> = T extends string ? R<T & {x:1}> : number;'],
 ['mapped','type R<T> = T extends string ? R<{[P in keyof T]: T[P]}> : number;'],
 ['union','type R<T> = T extends string ? R<T | {x:1}> : number;'],
 ['indexed','type R<T> = T extends {x:unknown} ? R<T["x"]> : number;'],
 ['alternating','type A<T> = T extends string ? B<{[P in keyof T]:T[P]}> : number; type B<T> = T extends string ? A<{[P in keyof T]:T[P]}> : number; type R<T> = A<T>;'],
];
const answers = [];
for(const [name,alias] of families) {
 const directory=path.join(root,`repeat-${name}`); fs.mkdirSync(directory,{recursive:true});
 const source = `${alias}\ninterface Host<X extends R<X>> { value: X; }\n`;
 fs.writeFileSync(path.join(directory,'cases.ts'),source);
 const config=path.join(directory,'tsconfig.json'); fs.writeFileSync(config,JSON.stringify({compilerOptions:{strict:true,noLib:true,noEmit:true,target:'esnext'},files:['cases.ts']})+'\n');
 const answer=await compilerHookOracle(config,{at:{file:'cases.ts',line:2,column:source.split('\n')[1].indexOf('value: X')+'value: '.length+1},properties:false});
 answers.push({name,source,answer}); console.log(JSON.stringify({name,base:answer.baseConstraint,after:answer.afterBaseConstraint}));
}
fs.writeFileSync(path.join(root,'repeat-answers.json'),JSON.stringify({sequence:'fresh selected X; base, resolved sentinel, guarded/raw constraints; summary afterward',answers},null,2)+'\n');

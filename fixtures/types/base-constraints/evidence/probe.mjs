import fs from 'node:fs';
import path from 'node:path';
import { compilerHookOracle } from './oracle.mjs';
const directory = path.resolve('.cc/temp/types-base-constraints');
const answers = [];
for (const count of [9,10,11,49,50,51,52]) {
  const here = path.join(directory, `chain-${count}`);
  fs.mkdirSync(here, {recursive:true});
  const parameters = Array.from({length:count}, (_, i) => `T${i} extends ${i+1<count ? `T${i+1}` : '"ready"'}`).join(', ');
  const source = `interface Chain<${parameters}> { value: T0; }\n`;
  fs.writeFileSync(path.join(here, 'cases.ts'), source);
  const config = path.join(here, 'tsconfig.json');
  fs.writeFileSync(config, JSON.stringify({compilerOptions:{strict:true,noLib:true,noEmit:true,target:'esnext'},files:['cases.ts']})+'\n');
  const offset = source.indexOf('value: T0') + 'value: '.length;
  const answer = await compilerHookOracle(config,{at:{file:'cases.ts',line:1,column:offset+1},properties:false});
  answers.push({count,source,request:{at:{file:'cases.ts',line:1,column:offset+1},properties:false},answer});
  console.log(JSON.stringify({count,answer:answer.baseConstraint,depth:answer.afterBaseConstraint?.counters?.baseConstraintDepth,diagnostics:answer.afterBaseConstraint?.diagnosticCodes}));
}
fs.writeFileSync(path.join(directory,'answers.json'),JSON.stringify({sequence:'fresh selected type; base, resolved cache identity, guarded declared constraint, raw worker, then summary; no diagnostics pass',answers},null,2)+'\n');

import fs from 'node:fs';import path from 'node:path';
import { compilerHookOracle } from '../../tools/type-oracle.mjs';
import { lineStarts, position } from '../../tools/semantic-oracle.mjs';
const root=path.resolve('fixtures/types/json-array-targets'),rows=[];
for(const name of ['missing','value-only','type-alias','enum','arity-zero','arity-two','script-alias','merged-optional-two','valid-empty','global-augmentation','stdlib']){
 const directory=path.join(root,name),source=fs.readFileSync(path.join(directory,'consumer.ts'),'utf8'),annotation='/*#numbers*/';
 const at={file:'consumer.ts',...position(lineStarts(source),source.indexOf(annotation)+annotation.length)};
 const answer=await compilerHookOracle(path.join(directory,'strict-1-exact-0-unchecked-0.json'),{at});
 rows.push({case:name,at,answer});console.log(JSON.stringify({case:name,summary:answer.summary,errorType:answer.errorType,before:answer.beforeSummary.diagnosticCodes,after:answer.diagnosticCodes}));
}
fs.writeFileSync('.cc/temp/json-native-global-array-hook.json',JSON.stringify(rows,null,2)+'\n');

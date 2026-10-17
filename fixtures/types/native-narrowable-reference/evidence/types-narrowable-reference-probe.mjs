import fs from 'node:fs';
import path from 'node:path';
import {typeOracle} from '../../tools/type-oracle.mjs';
import {compilerHookOracle} from './types-narrowable-reference/oracle.mjs';
import {lineStarts,position} from '../../tools/semantic-oracle.mjs';
const here=path.resolve('.cc/temp/types-narrowable-reference'),root=path.resolve('fixtures/types/native-narrowable-reference');
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8'),starts=lineStarts(source),rows=[];
const selected=marker=>({file:'cases.ts',...position(starts,source.indexOf(`/*#${marker}*/`)+marker.length+5)});
const markers=[...source.matchAll(/\/\*#(\w+)\*\//g)].map(match=>match[1]);
for(const config of ['tsconfig.json','loose.json'])for(const marker of markers){
 const request={at:selected(marker),diagnostics:false};
 const answer=await typeOracle(path.join(root,config),request);
 rows.push({config,marker,request,answer});
 process.stdout.write(JSON.stringify({config,marker,summary:answer.summary,errorType:answer.errorType})+'\n');
 fs.writeFileSync(path.join(here,'api-answers.json'),JSON.stringify(rows,null,2)+'\n');
}
const hooks=[];
for(const config of ['tsconfig.json','loose.json'])for(const marker of ['unconstrained','primitiveMissingContext','unionStatement','primitiveContext','genericContext','missingConstraint','genericElementReceiver'])for(const mode of ['normal','inferential']){
 const request={at:selected(marker),operation:'type',question:'value',history:'cold',narrowMode:mode};
 const answer=await compilerHookOracle(path.join(root,config),request);
 hooks.push({config,marker,request,answer});
 process.stdout.write(JSON.stringify({config,marker,mode,narrowable:answer.narrowable})+'\n');
 fs.writeFileSync(path.join(here,'hook-answers.json'),JSON.stringify(hooks,null,2)+'\n');
}

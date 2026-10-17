import fs from 'node:fs';
import path from 'node:path';
import {compilerHookOracle} from './types-loop-cache/oracle.mjs';
import {lineStarts,position} from '../../tools/semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-loop-cache'),source=fs.readFileSync(path.join(root,'loops.ts'),'utf8'),starts=lineStarts(source),rows=[];
for(const config of ['tsconfig.json','loose.json'])for(const marker of ['nestedAfter','continuesAfter','innerHead','innerExit','assignmentRhs','nestedRhs','backedgeInnerHead','declaredEntryAfter']){
 const offset=source.indexOf(`/*#${marker}*/`)+marker.length+5;
 const request={at:{file:'loops.ts',...position(starts,offset)},operation:'type'};
 const answer=await compilerHookOracle(path.join(root,config),request);
 rows.push({config,marker,request,answer});
 process.stdout.write(JSON.stringify({config,marker,summary:answer.summary,identity:answer.loopIdentity,beforeSummary:answer.beforeSummary})+'\n');
 fs.writeFileSync('.cc/temp/types-loop-cache/hook-answers.json',JSON.stringify(rows,null,2)+'\n');
}

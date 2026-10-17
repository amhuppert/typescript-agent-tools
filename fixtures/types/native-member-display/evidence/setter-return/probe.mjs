import fs from 'node:fs';
import path from 'node:path';
import { compilerHookOracle } from './oracle.mjs';
import { lineStarts, position } from '../../../tools/semantic-oracle.mjs';
const directory=path.resolve('.cc/temp/types-setter-return'),text=fs.readFileSync(path.join(directory,'cases.ts'),'utf8'),rows=[];
for(const config of ['strict.json','loose.json'])for(const marker of ['bodyless','paired','implemented']) {
 const offset=text.indexOf('/*'+marker+'*/')+marker.length+4;
 const request={at:{file:'cases.ts',...position(lineStarts(text),offset)},operation:'type'};
 const answer=await compilerHookOracle(path.join(directory,config),request);
 rows.push({config,marker,request,answer});
 process.stdout.write(JSON.stringify({config,marker,return:answer.setterReturn})+'\n');
 fs.writeFileSync(path.join(directory,'answers.json'),JSON.stringify(rows,null,2)+'\n');
}

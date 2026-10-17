import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { typeOracle } from '../../../tools/type-oracle.mjs';
import { lineStarts, position } from '../../../tools/semantic-oracle.mjs';
const root=fileURLToPath(new URL('./',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const markers=[...source.matchAll(/\/\*#([^*]+)\*\/ /g)].map(match=>({marker:match[1],offset:match.index+match[0].length}));
const rows=[];
for(const config of ['strict.json','loose.json']) {
 for(const {marker,offset} of markers) {
  const answer=await typeOracle(path.join(root,config),{at:{file:'cases.ts',...position(lineStarts(source),offset)},properties:false});
  rows.push({config,marker,request:{file:'cases.ts',...position(lineStarts(source),offset)},answer});
  process.stdout.write(`${config}\t${marker}\t${answer.kind}\t${answer.summary}\t${answer.errorType}\n`);
 }
}
fs.writeFileSync(path.join(root,'answers.json'),JSON.stringify(rows,null,2)+'\n');

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { typeOracle } from '../../../tools/type-oracle.mjs';
import { lineStarts, position } from '../../../tools/semantic-oracle.mjs';
const directory=fileURLToPath(new URL('./',import.meta.url));
const source=fs.readFileSync(path.join(directory,'cases.ts'),'utf8');
const at={file:'cases.ts',...position(lineStarts(source),source.indexOf('next: Missing'))};
const rows=[];
for(const config of ['strict.json','exact.json','loose.json']){
  const answer=await typeOracle(path.join(directory,config),{at,properties:false});
  rows.push({config,request:{at},answer});
  process.stdout.write(JSON.stringify({config,kind:answer.kind,summary:answer.summary,errorType:answer.errorType})+'\n');
}
fs.writeFileSync(path.join(directory,'child-answers.json'),JSON.stringify(rows,null,2)+'\n');

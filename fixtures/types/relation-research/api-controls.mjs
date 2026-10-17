import fs from 'node:fs';
import path from 'node:path';
import {typeOracle} from '../../../tools/type-oracle.mjs';
import {lineStarts,position} from '../../../tools/semantic-oracle.mjs';
process.env.TYPE_SCRIPT_SEMANTIC_API ??= '/Users/alex/github/typescript-agent-tools/.cache/compiler-oracles/typescript-7.0.2/package';
process.env.TYPE_SCRIPT_NATIVE ??= '/Users/alex/github/typescript-agent-tools/.cache/compiler-oracles/typescript-darwin-x64-7.0.2/package/lib/tsc';
const root=path.resolve('.cc/temp/types-relation-limits');
const results=[];
for(const name of ['allowance-1998-all','allowance-1999-all','allowance-2000-all']) {
  const directory=path.join(root,name);
  const text=fs.readFileSync(path.join(directory,'cases.ts'),'utf8');
  const offset=text.indexOf('/*#query*/')+'/*#query*/'.length;
  const answer=await typeOracle(path.join(directory,'tsconfig.json'),{at:{file:'cases.ts',...position(lineStarts(text),offset)}});
  const row={name,request:{at:{file:'cases.ts',...position(lineStarts(text),offset)}},answer};
  results.push(row);console.log(JSON.stringify({name,...answer}));
}
fs.writeFileSync(path.join(root,'api-controls.json'),JSON.stringify(results,null,2)+'\n');

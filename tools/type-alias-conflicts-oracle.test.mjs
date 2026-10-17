import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';

test('native alias conflict locations and suppression belong to the alias', async()=>{
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'native-alias-conflicts-'));
 const rows=[];
 try {
  fs.writeFileSync(path.join(root,'tsconfig.json'),JSON.stringify({compilerOptions:{noLib:true,module:'CommonJS'},files:['app.ts']}));
  fs.writeFileSync(path.join(root,'lib.ts'),'export const x = 1; export { x as original }; export default x;\n');
  fs.writeFileSync(path.join(root,'eq.ts'),'declare namespace target { const q: number; } export = target;\n');
  for(const [statement,column] of [
   ['import { x } from "./lib";',10],
   ['import { original as x } from "./lib";',10],
   ['import x from "./lib";',8],
   ['import * as x from "./lib";',13],
   ['import x = require("./eq");',1],
  ]) for(const aliasFirst of [true,false]) for(const suppressed of ['none','alias','local']){
   const alias=(suppressed==='alias'?'// @ts-ignore\n':'')+statement;
   const local=(suppressed==='local'?'// @ts-ignore\n':'')+'const x = 1;';
   const source=(aliasFirst?[alias,local]:[local,alias]).join('\n')+'\nconst probe = 1;\nprobe;\n';
   fs.writeFileSync(path.join(root,'app.ts'),source);
   const line=source.slice(0,source.indexOf(statement)).split('\n').length;
   const answer=await typeOracle(path.join(root,'tsconfig.json'),{at:{file:'app.ts',line:source.split('\n').length-1,column:1},diagnostics:true});
   const conflicts=answer.diagnostics.filter(diagnostic=>diagnostic.code===2440);
   rows.push({statement,aliasFirst,suppressed,conflicts});
   fs.mkdirSync('.cc/temp',{recursive:true});
   fs.writeFileSync('.cc/temp/native-alias-conflicts.json',JSON.stringify(rows,null,2));
   assert.equal(answer.compiler,'7.0.2');
   assert.equal(conflicts.length,suppressed==='alias'?0:1,JSON.stringify(rows.at(-1)));
   if(conflicts.length) assert.deepEqual(conflicts[0].location.start,{line,column},JSON.stringify(rows.at(-1)));
  }
  fs.writeFileSync(path.join(root,'app.ts'),'import { x as a, x as b } from "./lib";\nconst b = 1;\nconst a = 1;\nconst probe = 1;\nprobe;\n');
  const ordered=await typeOracle(path.join(root,'tsconfig.json'),{at:{file:'app.ts',line:5,column:1},diagnostics:true});
  const conflicts=ordered.diagnostics.filter(diagnostic=>diagnostic.code===2440);
  rows.push({aliasOrder:conflicts});
  assert.deepEqual(conflicts.map(diagnostic=>diagnostic.location.start),[{line:1,column:10},{line:1,column:18}]);
  fs.mkdirSync('.cc/temp',{recursive:true});
  fs.writeFileSync('.cc/temp/native-alias-conflicts.json',JSON.stringify(rows,null,2));
 } finally { fs.rmSync(root,{recursive:true,force:true}); }
});

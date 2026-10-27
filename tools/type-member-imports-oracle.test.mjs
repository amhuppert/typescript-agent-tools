import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';

const root=fileURLToPath(new URL('../fixtures/types/member-imports/',import.meta.url));
const queries=[['factory.ts','equalsMember'],['factory.ts','equalsExport'],
 ['default.ts','defaultMember'],['default.ts','defaultExport']];
const sources=Object.fromEntries(['factory.ts','default.ts','barrel.ts','use.ts','consumer.test.ts']
 .map(file=>[file,fs.readFileSync(path.join(root,file),'utf8')]));
function loc(file,marker,length){
 const source=sources[file],tag=`/*#${marker}*/`,offset=source.indexOf(tag)+tag.length;
 assert.ok(offset>=tag.length,marker);
 return {file,start:position(lineStarts(source),offset),end:position(lineStarts(source),offset+length)};
}
const origin=(file,marker,length,kind)=>({kind,location:loc(file,marker,length)});
const group=(origin,occurrences)=>({origins:[origin],occurrences});
const expectedEquals=[
 group(origin('consumer.test.ts','testEqualsAlias',7,'ImportEqualsDeclaration'),
  ['testEqualsAlias','testEqualsCall'].map(m=>loc('consumer.test.ts',m,7))),
 group(origin('factory.ts','equalsMember',3,'MethodDeclaration'),
  ['equalsMember','equalsExport'].map(m=>loc('factory.ts',m,3))),
 group(origin('use.ts','equalsAlias',7,'ImportEqualsDeclaration'),
  ['equalsAlias','equalsCall','equalsSaved'].map(m=>loc('use.ts',m,7))),
];
const expectedDefault=[
 group(origin('barrel.ts','reexportName',5,'ExportSpecifier'),
  [loc('barrel.ts','reexportName',5),loc('use.ts','namedSource',5)]),
 group(origin('consumer.test.ts','testDefaultAlias',6,'ImportClause'),
  ['testDefaultAlias','testDefaultCall'].map(m=>loc('consumer.test.ts',m,6))),
 group(origin('default.ts','defaultMember',3,'MethodDeclaration'),
  [loc('barrel.ts','reexportSource',7),loc('default.ts','defaultMember',3),loc('default.ts','defaultExport',3)]),
 group(origin('use.ts','defaultAlias',6,'ImportClause'),
  ['defaultAlias','defaultCall'].map(m=>loc('use.ts',m,6))),
 group(origin('use.ts','namedAlias',7,'ImportSpecifier'),
  ['namedAlias','renamedCall'].map(m=>loc('use.ts',m,7))),
];
test('native member searches follow export assignments, aliases and reexports',async()=>{
 const rows=[];
 const configs=['tsconfig.json','nocheck.json','loose.json','loose-nocheck.json',
  ...[16,64].flatMap(count=>['strict','nocheck','loose','loose-nocheck'].map(profile=>`resource-${profile}-${count}.json`))];
 for(const config of configs) for(const [file,marker] of queries){
  const source=fs.readFileSync(path.join(root,file),'utf8'),tag=`/*#${marker}*/`;
  const answer=await compilerHookOracle(path.join(root,config),{operation:'member',
   at:{file,...position(lineStarts(source),source.indexOf(tag)+tag.length)}});
  rows.push({config,file,marker,answer});
 }
 fs.writeFileSync(scratchPath('member-imports-native.json'),JSON.stringify(rows,null,2));
 assert.equal(rows.length,48);
 for(const row of rows) {
  assert.deepEqual(row.answer.demandFailures,[],row.marker);
  assert.deepEqual(row.answer.diagnosticCodes,[],row.marker);
  assert.deepEqual(row.answer.groups,row.marker.startsWith('equals')?expectedEquals:expectedDefault,row.marker);
 }
});

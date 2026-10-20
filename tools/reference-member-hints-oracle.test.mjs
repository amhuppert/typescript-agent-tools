import assert from 'node:assert/strict';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';

// The member searches that reference text's member hints and the `--file`
// filter lead to (tests/reference_text.rs, tests/reference_filters.rs), as
// the pinned TypeScript 7.0.2 checker answers them; the CLI tests expect the
// same declarations and occurrences.
const fixtures=fileURLToPath(new URL('../fixtures/types/',import.meta.url));
const search=(fixture,file,line,column)=>compilerHookOracle(path.join(fixtures,fixture,'tsconfig.json'),{operation:'member',at:{file,line,column}});
const at=(file,line,column,length)=>({file,start:{line,column},end:{line,column:column+length}});
const group=(kind,origin,occurrences)=>({origins:[{kind,location:origin}],occurrences});

test('member hints: an anonymous default class member, a static member, and parameter properties',async()=>{
 const anonymous=await search('reference-member-hints','anon.ts',1,24);
 assert.deepEqual(anonymous.demandFailures,[]);
 assert.deepEqual(anonymous.groups,[group('MethodDeclaration',at('anon.ts',1,24,3),[at('anon.ts',1,24,3),at('use.ts',4,12,3)])]);
 // The static member, apart from the instance member sharing its name.
 const statics=await search('reference-member-hints','both.ts',1,28);
 assert.deepEqual(statics.demandFailures,[]);
 assert.deepEqual(statics.groups,[group('MethodDeclaration',at('both.ts',1,28,3),[at('both.ts',1,28,3),at('use.ts',5,6,3)])]);
 // A parameter property's member is reached through its modifier; its name
 // is the parameter, which the property's uses do not reference.
 const property=await search('reference-member-hints','parameter.ts',1,38);
 assert.deepEqual(property.demandFailures,[]);
 assert.deepEqual(property.groups,[group('Parameter',at('parameter.ts',1,45,3),[at('parameter.ts',1,45,3),at('use.ts',7,18,3)])]);
 const parameter=await search('reference-member-hints','parameter.ts',1,45);
 assert.deepEqual(parameter.groups,[group('Parameter',at('parameter.ts',1,45,3),[at('parameter.ts',1,45,3)])]);
 // Beside a static member of its name, and in an anonymous class, the
 // property is still its own member, reached through its modifier.
 const shared=await search('reference-member-hints','shared.ts',1,35);
 assert.deepEqual(shared.demandFailures,[]);
 assert.deepEqual(shared.groups,[group('Parameter',at('shared.ts',1,42,3),[at('shared.ts',1,42,3),at('use.ts',10,15,3)])]);
 const anonymousProperty=await search('reference-member-hints','anonparam.ts',1,36);
 assert.deepEqual(anonymousProperty.demandFailures,[]);
 assert.deepEqual(anonymousProperty.groups,[group('Parameter',at('anonparam.ts',1,43,3),[at('anonparam.ts',1,43,3),at('use.ts',12,18,3)])]);
});

test('file filter: a member used in two files, with a use on an unknown receiver',async()=>{
 const answer=await search('reference-file-filter','lib.ts',1,24);
 assert.deepEqual(answer.demandFailures,[]);
 assert.deepEqual(answer.groups,[group('PropertySignature',at('lib.ts',1,24,5),[at('a.ts',3,3,5),at('b.ts',3,3,5),at('lib.ts',1,24,5)])]);
 // The unknown receiver's candidate has no symbol: the tool reports it as a gap.
 const unknown=answer.candidateObservations.filter(row=>row.location.file==='d.ts');
 assert.deepEqual(unknown.map(row=>row.location),[at('d.ts',3,17,5)]);
 assert.ok(unknown.every(row=>row.symbolFlags===0&&row.origins.length===0));
});

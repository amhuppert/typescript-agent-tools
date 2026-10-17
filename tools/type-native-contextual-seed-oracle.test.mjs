import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-contextual-seed');
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const offset=marker=>source.indexOf(`/*#${marker}*/`)+marker.length+5;
const loc=marker=>({file:'cases.ts',start:position(lineStarts(source),offset(marker)),end:position(lineStarts(source),offset(marker)+7)});
test('contextual seed relates every occurrence to its contextual declaration without an empty own group',async()=>{
 const answer=await compilerHookOracle(path.join(root,'tsconfig.json'),{operation:'member',at:{file:'cases.ts',...loc('seed').start}});
 assert.deepEqual(answer.groups,[{origins:[{kind:'MethodSignature',location:loc('origin')}],occurrences:['origin','seed','call'].map(loc)}]);
 assert.deepEqual(answer.demandFailures,[]);
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle,compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';

const root=fileURLToPath(new URL('../fixtures/types/native-mapped-types/',import.meta.url));
export function mappedOracleTests(files) {
for(const file of files) {
  test(`native mapped ${file} acquisition and members`,async()=>{
    const source=fs.readFileSync(path.join(root,`${file}.ts`),'utf8');
    const rows=[];
    for(const config of ['tsconfig','loose','exact']) {
      for(const match of source.matchAll(/\/\*#([^*]+)\*\//g)) {
        const at={file:`${file}.ts`,...position(lineStarts(source),match.index+match[0].length)};
        const summary=await typeOracle(path.join(root,`${config}.json`),{at,properties:false});
        const hook=await compilerHookOracle(path.join(root,`${config}.json`),{operation:'type',at});
        assert.equal(summary.compiler,'7.0.2');
        assert.equal(summary.summary,hook.summary,`${config}/${file}/${match[1]}`);
        const expanded=await typeOracle(path.join(root,`${config}.json`),{at,properties:true});
        // Native generated symbol suffixes are process incidental. This case
        // pins two distinct keys and their ordered values/origins, not counters.
        if(match[1]==='uniqueKeys') {
          assert.equal(expanded.properties.length,2);
          assert.notEqual(expanded.properties[0].name,expanded.properties[1].name);
          expanded.properties.forEach((property,index)=>{property.name=`unique:${index}`;});
        }
        rows.push({config,name:match[1],summary:summary.summary,errorType:summary.errorType,codes:hook.beforeSummary.diagnosticCodes,
          properties:expanded.properties.map(({name,summary,errorType,origins})=>({name,summary,errorType,origins}))});
      }
    }
    const capture=`.cc/temp/types-mapped-${file}-native.json`;
    fs.mkdirSync('.cc/temp',{recursive:true});
    fs.writeFileSync(capture,JSON.stringify(rows,null,2)+'\n');
    const expected=path.join(root,`expected-${file}.json`);
    if(fs.existsSync(expected)) assert.deepEqual(rows,JSON.parse(fs.readFileSync(expected,'utf8')));
    else assert.ok(rows.length>0,`capture missing ${file}`);
  });
}
}

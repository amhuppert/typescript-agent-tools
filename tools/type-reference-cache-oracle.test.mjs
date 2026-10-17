import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-generic-aliases/', import.meta.url));
const source = fs.readFileSync(path.join(root,'imported.ts'),'utf8');
for (const history of ['baseline','cold']) {
  test(`native TypeReference nodes cache repeated imported-alias argument acquisition (${history})`, async () => {
    const tag = '/*#nestedImported*/';
    const answer = await compilerHookOracle(path.join(root,'tsconfig.json'), {
      at:{file:'imported.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)},
      operation:'type',history,
    });
    assert.equal(answer.summary,'Exported<Exported<Exported<string>>>');
    const counters = answer.beforeSummary.counters;
    assert.deepEqual({hit:counters.typeReferenceCacheHit,miss:counters.typeReferenceCacheMiss},
      {hit:2,miss:3});
    assert.deepEqual(answer.beforeSummary.demandFailures,[]);
  });
}

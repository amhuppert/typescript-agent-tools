import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-const-tuples/', import.meta.url));
const source = fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected = JSON.parse(fs.readFileSync(path.join(root,'expected.json'),'utf8'));
test('native const type-variable recognition through generic variadic tuples', async t => {
  const results=[];
  for (const config of ['tsconfig.json','loose.json','exact.json']) for(const marker of Object.keys(expected)) {
    const tag=`/*#${marker}*/`;
    const at={file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)};
    const answer=await typeOracle(path.join(root,config),{at});
    assert.equal(answer.compiler,'7.0.2');
    results.push({config,marker,summary:answer.summary,error:answer.errorType});
  }
  t.diagnostic(JSON.stringify(results));
  for (const result of results) {
    assert.equal(result.error,false);
    assert.equal(result.summary,expected[result.marker],`${result.config}/${result.marker}`);
  }
});

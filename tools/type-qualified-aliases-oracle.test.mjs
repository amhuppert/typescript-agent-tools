import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-qualified-aliases/', import.meta.url));
const source = fs.readFileSync(path.join(root,'cases.ts'),'utf8');
const expected = [
  ['direct','Original<"direct">',false], ['named','Original<"named">',true],
  ['typeOnly','Original<"typeOnly">',true], ['nested','Original<"nested">',false],
  ['aliasFirst','Original<"first">',true], ['directFirst','Original<"first">',false],
  ['cycle','Original<"cycle">',true], ['equals','Original<"equals">',false],
  ['own','Own<"own">',false], ['localAlias','Original<"alias">',true],
  ['shadow','Original<"shadow">',true], ['dependency','External<"dependency">',true],
  ['missing','MissingShadow<"missing">',true],
];
test('native Alias-only entity lookup preserves selected raw export and lexical identities', async () => {
  const records=[];
  for (const config of ['tsconfig.json','loose.json','exact.json']) {
    for (const [marker] of expected) {
      const tag=`/*#${marker}*/`;
      const at={file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)};
      const answer=await compilerHookOracle(path.join(root,config),{at,operation:'type'});
      const aliases=Object.entries(answer.beforeSummary.observations)
        .filter(([key])=>key.startsWith('typeReferenceAlias:')).map(([,value])=>value);
      records.push({config,marker,summary:answer.summary,errorType:answer.errorType,aliases,codes:answer.beforeSummary.diagnosticCodes});
      fs.writeFileSync('.cc/temp/native-qualified-aliases-observations.json',JSON.stringify(records,null,2));
    }
  }
  for (const record of records) {
    const [,summary,alias]=expected.find(([marker])=>marker===record.marker);
    assert.equal(record.summary,summary,`${record.config}/${record.marker}`);
    assert.equal(record.errorType,false,`${record.config}/${record.marker}`);
    assert.equal(record.aliases.length,1,`${record.config}/${record.marker} actual alias lookup count`);
    assert.equal(record.aliases[0].alias,alias,`${record.config}/${record.marker} actual raw Alias selection`);
  }
});

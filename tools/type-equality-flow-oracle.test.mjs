import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-equality-flow/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const cases = [
  ...['unrelatedStrict','unrelatedLoose','unrelatedNotStrict','unrelatedNotLoose','selfStrict','selfLoose','selfNotStrict','selfNotLoose','booleanRight','booleanLeft','typeofSame','typeofSwapped','typeofOther'].map(name => [name,'number',true]),
  ['anySkips','any',true], ['anySwappedSkips','any',true], ['matchedMissing','number',true],
  ['literalChanges','1',false], ['swappedChanges','1',false], ['negativeChanges','2',false],
  ['nullableChanges','number',false], ['nullableLoose','never',false],
  ['optionalSame','{ value: number; } | undefined',true], ['optionalChanges','{ value: number; }',false],
  ['typeofChanges','number',false], ['discriminantChanges','{ kind: "a"; value: number; }',false],
  ['constructorInequality','HasConstructor',true], ['constructorNumber','HasConstructor',true],
];
test('native equality preserves exact unchanged flow identities and ordered acquired errors', async () => {
  const observations = [];
  for (const config of ['tsconfig.json','loose.json','exact.json']) for (const [marker, expected, same] of cases) {
    const tag = `/*#${marker}*/`;
    const offset = source.indexOf(tag) + tag.length;
    const answer = await compilerHookOracle(path.join(root, config), {
      at: {file:'cases.ts', ...position(lineStarts(source), offset)}, properties:false,
    });
    const flow = Object.entries(answer.beforeSummary.observations)
      .filter(([key,value]) => key.startsWith('referenceFlow:') && value.referenceEnd === offset + 1)
      .map(([,value]) => value);
    observations.push({config,marker,answer,flow});
    fs.writeFileSync('.cc/temp/native-equality-flow-observations.json', JSON.stringify(observations,null,2));
    const loose = config === 'loose.json';
    const summary = loose && marker === 'nullableLoose' ? 'number'
      : loose && marker === 'optionalSame' ? '{ value: number; }' : expected;
    const unchanged = same || loose && ['nullableChanges','nullableLoose','optionalChanges'].includes(marker);
    assert.equal(answer.summary,summary,`${config}/${marker}`);
    assert.equal(answer.errorType,false,`${config}/${marker}`);
    assert.ok(flow.length > 0,`${config}/${marker} missing actual flow return witness`);
    assert.equal(flow.at(-1).sameDeclared,unchanged,`${config}/${marker} declared identity`);
    assert.equal(flow.at(-1).sameInitial,unchanged,`${config}/${marker} initial identity`);
    if (marker === 'matchedMissing') {
      assert.ok(Object.entries(answer.beforeSummary.observations).some(([key,value]) => key.startsWith('referenceFlow:') && value.errorType), 'opposite reference consumed native error type');
    } else if (['anySkips','anySwappedSkips','constructorInequality'].includes(marker)) {
      assert.ok(!Object.entries(answer.beforeSummary.observations).some(([key,value]) => key.startsWith('referenceFlow:') && value.errorType), 'native gate skipped opposite error type');
    }
  }
});

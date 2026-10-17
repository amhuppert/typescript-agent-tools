import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
import {assertFixtureCapture} from './type-oracle-fixture.mjs';

const cases = [
  {name:'reference gaps',directory:'native-reference-gap-resources',golden:'expected.json',
    select:row=>row.variant.tag==='ascii'&&row.variant.width===16&&row.variant.profile==='strict',operation:'member',marker:'/*#declaration*/'},
  {name:'named declarations',directory:'native-named-declaration-resources',golden:'expected.json',
    select:row=>row.variant.layout==='single'&&row.variant.width===16&&row.variant.profile==='strict',operation:'member'},
  {name:'late reference normalization',directory:'native-late-merge-resources',golden:'expected-references.json',
    select:row=>row.width===16&&row.profile==='strict'&&row.seed==='only-late-use',operation:'member'},
  {name:'accessor values',directory:'native-accessor-declaration-resources',golden:'expected.json',
    select:row=>row.variant.layout==='plain'&&row.variant.width===16&&row.variant.profile==='strict'&&row.mode==='read',operation:'type'},
  {name:'accessor signatures',directory:'native-accessor-declaration-resources',golden:'expected-signatures.json',
    select:row=>row.profile==='strict'&&row.key==='literal',operation:'type'},
];

for(const entry of cases) test(`frozen ${entry.name} replay in another directory`,async t=>{
  const original=path.resolve('fixtures/types',entry.directory);
  const directory=fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(),'native-fixture-replay-')));
  t.after(()=>fs.rmSync(directory,{recursive:true,force:true}));
  const root=path.join(directory,'relocated-fixture');
  fs.cpSync(original,root,{recursive:true});
  const frozen=JSON.parse(fs.readFileSync(path.join(original,entry.golden),'utf8')).find(entry.select);
  assert.ok(frozen,entry.name);
  let at=frozen.at;
  if(!at) {
    const marker=entry.marker??'/*#chosen*/';
    at={file:frozen.variant.file,...position(lineStarts(frozen.source),frozen.source.indexOf(marker)+marker.length)};
  }
  const config=frozen.variant?.config??frozen.config??`${frozen.profile}.json`;
  const answer=await compilerHookOracle(path.join(root,config),{operation:entry.operation,at});
  const actual={...structuredClone(frozen),answer};
  if(frozen.normalizedGroups) {
    const related=answer.candidateObservations.filter(row=>row.relatedOrigins.length>0);
    actual.normalizedGroups=related.length?[{origins:related[0].relatedOrigins,occurrences:related.map(row=>row.nativeRange)}]:[];
  }
  assertFixtureCapture([actual],[frozen],root);

  // Source bytes and arbitrary semantic strings may contain the old directory;
  // rebasing every string would incorrectly alter those identities.
  const oldRoot=path.dirname(frozen.answer.compilerOptions.configFilePath);
  const decorated=structuredClone(frozen), moved=structuredClone(actual);
  decorated.literalName=moved.literalName=oldRoot+'/literal-name';
  decorated.source=moved.source=`const pathName = ${JSON.stringify(oldRoot)};`;
  assertFixtureCapture([moved],[decorated],root);
  const changedSource=structuredClone(moved);
  changedSource.source=changedSource.source.replace(oldRoot,root);
  assert.throws(()=>assertFixtureCapture([changedSource],[decorated],root),assert.AssertionError);
  moved.literalName=root+'/literal-name';
  assert.throws(()=>assertFixtureCapture([moved],[decorated],root),assert.AssertionError);

  const changed=structuredClone(actual);
  changed.answer.checkerCount+=1;
  assert.throws(()=>assertFixtureCapture([changed],[frozen],root),assert.AssertionError);
  const wrongRoot=structuredClone(actual);
  wrongRoot.answer.compilerOptions.configFilePath=path.join(directory,'wrong-root',config);
  assert.throws(()=>assertFixtureCapture([wrongRoot],[frozen],root),assert.AssertionError);
  const wrongCase=structuredClone(actual);
  const input=wrongCase.answer.inputFiles[0];
  wrongCase.answer.inputFiles[0]=path.posix.join(path.posix.dirname(input),path.posix.basename(input).toUpperCase());
  assert.notEqual(wrongCase.answer.inputFiles[0],input);
  assert.throws(()=>assertFixtureCapture([wrongCase],[frozen],root),assert.AssertionError);

  const externalExpected=structuredClone(frozen),externalActual=structuredClone(actual);
  externalExpected.answer.inputFiles.push(oldRoot+'-sibling/external.d.ts');
  externalActual.answer.inputFiles.push(oldRoot+'-sibling/external.d.ts');
  assertFixtureCapture([externalActual],[externalExpected],root);
  externalActual.answer.inputFiles[externalActual.answer.inputFiles.length-1]=root+'-sibling/external.d.ts';
  assert.throws(()=>assertFixtureCapture([externalActual],[externalExpected],root),assert.AssertionError);
});

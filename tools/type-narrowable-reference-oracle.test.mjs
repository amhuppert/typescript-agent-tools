import assert from 'node:assert/strict';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {loadSemanticApi, lineStarts, position} from './semantic-oracle.mjs';

const root=fileURLToPath(new URL('../fixtures/types/native-narrowable-reference/',import.meta.url));
const unchanged=['unconstrained','primitiveConstraint','unionStatement','nullableStatement',
  'genericContext','primitiveMissingContext','missingConstraint','genericElementReceiver',
  'typeofOperand','uninitializedContext'];
for(const strict of [true,false]) test(`native reference substitution precedes contextual work (strict=${strict})`,async()=>{
  const native=await loadSemanticApi();
  assert.equal(native.version,'7.0.2');
  const config=path.join(root,strict?'tsconfig.json':'loose.json');
  const api=new native.API({tsserverPath:native.binary,cwd:root});
  let snapshot;
  try {
    snapshot=api.updateSnapshot({openProject:config});
    const project=snapshot.getProject(config);
    for(const rows of [project.program.getConfigFileParsingDiagnostics(),project.program.getProgramDiagnostics(),
      project.program.getSyntacticDiagnostics(),project.program.getBindDiagnostics()]) assert.deepEqual(rows,[]);
    const source=project.program.getSourceFile(path.join(root,'cases.ts')),checker=project.checker;
    // Every actual occurrence uses a fresh public request below. This separate
    // request proves canonical raw T identity without checking a context node.
    const offset=source.text.indexOf('/*#unconstrained*/')+'/*#unconstrained*/'.length;
    const node=native.getTokenAtPosition(source,offset);
    const actual=checker.getTypeAtLocation(node);
    const raw=checker.getTypeOfSymbol(checker.getSymbolAtLocation(node));
    assert.equal(actual.id,raw.id);
    assert.equal(actual.flags,native.TypeFlags.TypeParameter);
  } finally {try {snapshot?.dispose();}finally {api.close();}}
  const fs=await import('node:fs');
  const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8'),starts=lineStarts(source);
  const expected=new Map(unchanged.map(marker=>[marker,'T']));
  for(const [marker,summary] of [
    ['primitiveContext','string | number'], ['nullableContext',strict?'string | undefined':'T'],
    ['nonGenericMissingContext','string'],
    ['propertyReceiver','{ value: number; } | { value: string; }'],
    ['elementReceiver','{ value: number; } | { value: string; }'],
    ['callReceiver','(() => number) | (() => string)'],
    ['newReceiver','(new () => { value: number; }) | (new () => { value: string; })'],
    ['parenthesizedContext','string | number'],['satisfiesContext','string | number'],
    ['unionOfParameter',strict?'string | number | undefined':'number | T'],
  ])expected.set(marker,summary);
  for(const [marker,summary] of expected){
    const offset=source.indexOf(`/*#${marker}*/`)+marker.length+5;
    const answer=await typeOracle(config,{at:{file:'cases.ts',...position(starts,offset)},diagnostics:false});
    assert.equal(answer.summary,summary,marker);
    assert.equal(answer.errorType,false,marker);
    assert.equal(answer.kind,'expression',marker);
    assert.deepEqual(answer.prelude.map(row=>row.kind),
      marker==='callReceiver'?['CallExpression']:marker==='newReceiver'?['NewExpression']:[],marker);
  }
});

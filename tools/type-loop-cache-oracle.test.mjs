import assert from 'node:assert/strict';
import fs from 'node:fs';
import {createHash} from 'node:crypto';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {loadSemanticApi, lineStarts, position} from './semantic-oracle.mjs';

const root=fileURLToPath(new URL('../fixtures/types/native-loop-cache/',import.meta.url));
const changed=new Map([['narrowedAfter','number'],['doAfter','string'],
  ['shadowInner','string'],['shadowOuter','number']]);
for(const strict of [true,false]) test(`native loop cache preserves complete reference identities (strict=${strict})`,async()=>{
  const native=await loadSemanticApi();
  assert.equal(native.version,'7.0.2');
  const config=path.join(root,strict?'tsconfig.json':'loose.json');
  const text=fs.readFileSync(path.join(root,'loops.ts'),'utf8'),starts=lineStarts(text);
  for(const match of text.matchAll(/\/\*#(\w+)\*\//g)) {
    const marker=match[1],offset=match.index+match[0].length;
    const summary=changed.get(marker)??'string | number';
    const request={at:{file:'loops.ts',...position(starts,offset)},diagnostics:false};
    const answer=await typeOracle(config,request);
    assert.equal(answer.summary,summary,marker);
    assert.equal(answer.errorType,false,marker);
    assert.equal(answer.kind,'expression',marker);
    assert.equal(answer.sourceHash,createHash('sha256').update(text,'utf8').digest('hex'),marker);
    // A separate cold checker proves actual declared/selected identity; no
    // prior summary or whole-program diagnostic pass changes this demand.
    const api=new native.API({tsserverPath:native.binary,cwd:root});
    let snapshot;
    try {
      snapshot=api.updateSnapshot({openProject:config});
      const project=snapshot.getProject(config),checker=project.checker;
      for(const rows of [project.program.getConfigFileParsingDiagnostics(),project.program.getProgramDiagnostics(),
        project.program.getSyntacticDiagnostics(),project.program.getBindDiagnostics()])assert.deepEqual(rows,[]);
      const source=project.program.getSourceFile(path.join(root,'loops.ts'));
      const node=native.getTokenAtPosition(source,offset);
      const actual=checker.getTypeAtLocation(node);
      const declared=checker.getTypeOfSymbol(checker.getSymbolAtLocation(node));
      assert.equal(actual.isErrorType(),false,marker);
      assert.equal(actual.id===declared.id,!changed.has(marker),marker);
      assert.equal(checker.typeToString(actual),summary,marker);
    } finally {try{snapshot?.dispose();}finally{api.close();}}
  }
});

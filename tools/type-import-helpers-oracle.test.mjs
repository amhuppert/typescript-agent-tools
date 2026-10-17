import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';
import {typeOracle} from './type-oracle.mjs';

const fixtures=fileURLToPath(new URL('../fixtures/types/native-import-helpers/',import.meta.url));
const cases=JSON.parse(fs.readFileSync(path.join(fixtures,'cases.json'),'utf8'));
test('native helper imports load only the effective eligible inputs and remain cold when missing',async t=>{
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'tsparse-import-helpers-'));
 t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
 const semantic=await loadSemanticApi();
 const rows=[];
 for(const row of cases){
  const directory=path.join(root,row.name);fs.mkdirSync(directory);
  for(const file of ['helpers.d.ts','globals.d.ts'])fs.copyFileSync(path.join(fixtures,file),path.join(directory,file));
  fs.writeFileSync(path.join(directory,row.file),row.source);
  const config=path.join(directory,'tsconfig.json');
  fs.writeFileSync(config,JSON.stringify({compilerOptions:{noLib:true,types:[],moduleDetection:'legacy',paths:{tslib:['./helpers.d.ts']},...row.options},files:[row.file,'globals.d.ts']}));
  const api=new semantic.API({tsserverPath:semantic.binary,cwd:directory});
  let snapshot;
  try{
   snapshot=api.updateSnapshot({openProject:config});
   const project=snapshot.getProject(config);assert.ok(project);
   const loaded=project.program.getSourceFileNames().some(file=>file.endsWith('/helpers.d.ts'));
   assert.equal(loaded,row.loaded,row.name);
   const fatal=[...project.program.getConfigFileParsingDiagnostics(),...project.program.getProgramDiagnostics(),...project.program.getSyntacticDiagnostics()];
   assert.deepEqual(fatal,[],row.name);
   rows.push({name:row.name,loaded});
  }finally{try{snapshot?.dispose();}finally{api.close();}}
  if(row.file.endsWith('.json')){
   fs.writeFileSync(path.join(directory,'helpers.d.ts'),'export declare const broken: ;');
   const api=new semantic.API({tsserverPath:semantic.binary,cwd:directory});let snapshot;
   try{
    snapshot=api.updateSnapshot({openProject:config});
    const diagnostics=snapshot.getProject(config).program.getSyntacticDiagnostics();
    assert.equal(diagnostics.some(d=>d.code===1110),row.loaded,row.name);
   }finally{try{snapshot?.dispose();}finally{api.close();}}
   continue;
  }
  const offset=row.source.indexOf('selected:');
  const before=row.source.slice(0,offset).split('\n');
  const answer=await typeOracle(config,{at:{file:row.file,line:before.length,column:before.at(-1).length+1},properties:false});
  assert.equal(answer.summary,row.loaded?'27':'any',row.name);
  assert.equal(answer.errorType,!row.loaded,row.name);
  // The synthetic request alone does not produce a missing-module diagnostic,
  // even with the diagnostic policy for actual side-effect imports enabled.
  fs.unlinkSync(path.join(directory,'helpers.d.ts'));
  fs.writeFileSync(path.join(directory,row.file),(row.source.startsWith('export')?'export {};\n':'')+'declare let /*healthy*/ healthy: number;\n');
  const options=JSON.parse(fs.readFileSync(config,'utf8'));options.compilerOptions.noUncheckedSideEffectImports=true;
  fs.writeFileSync(config,JSON.stringify(options));
  const healthy=await typeOracle(config,{at:{file:row.file,line:row.source.startsWith('export')?2:1,column:25},properties:false});
  assert.equal(healthy.summary,'number',row.name);
  assert.equal(healthy.errorType,false,row.name);
 }
 fs.writeFileSync('.cc/temp/types-import-helpers-native.json',JSON.stringify(rows,null,2));
});

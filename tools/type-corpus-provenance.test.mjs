import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {execFileSync,spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';

// A dependency's lib reference distinguishes source provenance even though
// the same default-library source bytes and order are selected in both APIs.
for(const singleThreaded of [true]) test('native baseline and demand capture use the same source provenance (singleThreaded='+singleThreaded+')',async t=>{
  const root=fs.realpathSync(fs.mkdtempSync(path.resolve('.cc/temp/corpus-provenance-')));
  t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
  const sources={
    'src/lib/specs/execution-service.ts':'import "dep";\ninterface SpecWorkflowCleanupPort {\n  observe(): void;\n}\n',
    'src/lib/specs/workflow-cleanup-port.ts':'const value = {\n    observe: () => {},\n};\n',
    'src/lib/agent-gateway/route-handlers.ts':'export function createAgentGatewayHandlers() { return 42; }\n',
    'src/components/CollapsibleText.tsx':'interface CollapsibleTextProps {\n  maxCollapsedHeight?: number;\n}\n',
    'src/lib/notepads/service.ts':'function selected(modes:string[]) {\n  return modes.filter(mode => mode);\n}\n',
    'node_modules/dep/package.json':'{"name":"dep","version":"1.0.0","types":"index.d.ts"}',
    'node_modules/dep/index.d.ts':'/// <reference lib="es2015" />\nexport declare const value:42;\n',
    'package.json':'{"private":true}',
    'tsconfig.json':JSON.stringify({compilerOptions:{singleThreaded,strict:true,module:'Preserve',moduleResolution:'Bundler',types:[],target:'ES2022',lib:['dom','dom.iterable','esnext']},
      files:['src/lib/specs/execution-service.ts','src/lib/specs/workflow-cleanup-port.ts',
        'src/lib/agent-gateway/route-handlers.ts','src/components/CollapsibleText.tsx','src/lib/notepads/service.ts']}),
  };
  for(const [file,text]of Object.entries(sources)){
    fs.mkdirSync(path.dirname(path.join(root,file)),{recursive:true});fs.writeFileSync(path.join(root,file),text);
  }
  execFileSync('git',['init','-q',root]);
  execFileSync('git',['-C',root,'add','src','package.json','tsconfig.json']);
  execFileSync('git',['-C',root,'-c','user.name=Capture Test','-c','user.email=capture@example.invalid','commit','-q','-m','fixture']);
  const revision=execFileSync('git',['-C',root,'rev-parse','HEAD'],{encoding:'utf8'}).trim();
  const output=path.join(root,'manifest.json');
  const freeze=fileURLToPath(new URL('../fixtures/types/command-center/freeze.mjs',import.meta.url));
  const frozen=spawnSync(process.execPath,[freeze,root,output,'--revision='+revision],{encoding:'utf8'});
  assert.equal(frozen.status,0,frozen.stderr);
  const manifest=JSON.parse(fs.readFileSync(output));
  const selected=manifest.queries.find(row=>row.id==='annotated-member');
  let capturedInput, capturedOutput;
  const readFile=fs.readFileSync,writeFile=fs.writeFileSync;
  fs.writeFileSync=function(file,bytes,...args){
    if(typeof file==='string' && file.includes('/type-oracle-hook-') && path.basename(file)==='input.json')
      capturedInput=JSON.parse(bytes);
    return writeFile.call(this,file,bytes,...args);
  };
  fs.readFileSync=function(file,...args){
    const bytes=readFile.call(this,file,...args);
    if(typeof file==='string' && file.includes('/type-oracle-hook-') && path.basename(file)==='output.json')
      capturedOutput=JSON.parse(bytes);
    return bytes;
  };
  let answer;
  try {
    answer=await compilerHookOracle(path.join(root,'tsconfig.json'),{operation:'type',at:selected.at,question:selected.question});
  } finally {
    fs.readFileSync=readFile;fs.writeFileSync=writeFile;
    fs.writeFileSync('.cc/temp/stage66-native-program-provenance-transport-'+singleThreaded+'.json',
      JSON.stringify({api:capturedInput?.programFiles,native:capturedOutput?.programFiles},null,2)+'\n');
  }
  const files=new Map(manifest.files.map(row=>[row.file,row]));
  const expected=manifest.programFiles.map(file=>{
    const row=files.get(file);
    return {file:path.resolve(root,file),defaultLibrary:row.defaultLibrary,externalLibrary:row.externalLibrary};
  });
  fs.writeFileSync('.cc/temp/stage66-corpus-provenance-small.json',JSON.stringify({expected,actual:answer.programFiles},null,2)+'\n');
  assert.equal(answer.summary,'() => void');
  assert.equal(answer.errorType,false);
  assert.deepEqual(answer.programFiles,expected,
    'a baseline must preserve the exact demand capture provenance, including lib references from external packages');
});

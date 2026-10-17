// Archive the observed tools only if their original before/after hashes still match.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
const root=process.cwd();
const directory=path.join(root,'fixtures/types/native-member-display/evidence');
const observed=JSON.parse(fs.readFileSync(path.join(directory,'research-provenance.json'),'utf8'));
const hash=data=>createHash('sha256').update(data).digest('hex');
if(JSON.stringify(observed.before)!==JSON.stringify(observed.after))throw new Error('Original observed tooling changed during capture');
for(const row of observed.before){
 const bytes=fs.readFileSync(path.join(root,row.file));
 if(hash(bytes)!==row.sha256)throw new Error(`Current tool no longer matches observed tool: ${row.file}`);
 const target=path.join(directory,'captured-tooling',row.file);
 fs.mkdirSync(path.dirname(target),{recursive:true});
 fs.writeFileSync(target,bytes);
}
for(const file of ['tools/type-member-display-oracle.test.mjs','tools/corpus-selection.mjs','tools/vendor-typescript-libs.mjs']){
 const target=path.join(directory,'captured-tooling',file);
 fs.mkdirSync(path.dirname(target),{recursive:true});
 fs.copyFileSync(path.join(root,file),target);
}
const files=[];
function visit(folder){
 for(const entry of fs.readdirSync(folder,{withFileTypes:true})){
  const file=path.join(folder,entry.name);
  if(entry.isDirectory())visit(file);
  else if(file!==path.join(directory,'provenance.json')){
   const bytes=fs.readFileSync(file);
   files.push({file:path.relative(directory,file).split(path.sep).join('/'),bytes:bytes.length,sha256:hash(bytes)});
  }
 }
}
visit(directory);
files.sort((a,b)=>a.file<b.file?-1:a.file>b.file?1:0);
const inputs=['cases.ts','strict.json','exact.json','loose.json'].map(file=>{
 const bytes=fs.readFileSync(path.join(directory,'..',file));
 return {file,bytes:bytes.length,sha256:hash(bytes)};
});
fs.writeFileSync(path.join(directory,'provenance.json'),JSON.stringify({scope:'Original observed inline-display API/hook sessions and separately registered formal native suite',registeredRun:'vrun-06824310-0f87-4728-aa41-94cfbc8ccf10',nativeVersion:'7.0.2',nativeCommit:'2bd066d87f5bafd315be9f40889d0a60b9e58e0b',inputs,files},null,2)+'\n');

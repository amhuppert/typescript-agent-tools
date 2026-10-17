import fs from 'node:fs';import path from 'node:path';
import {loadSemanticApi,lineStarts,position} from '../../tools/semantic-oracle.mjs';
const root=path.resolve('fixtures/types/json-array-targets'),sem=await loadSemanticApi(),rows=[];
for(const name of ['missing','value-only','module-local','type-alias','enum','arity-zero','arity-two','script-alias','merged-optional-two']){
 const directory=path.join(root,name),config=path.join(directory,'strict-1-exact-0-unchecked-0.json'),api=new sem.API({tsserverPath:sem.binary,cwd:directory});let snapshot;
 try{snapshot=api.updateSnapshot({openProject:config});const p=snapshot.getProject(config),c=p.checker,s=p.program.getSourceFile(path.join(directory,'consumer.ts')),annotation='/*#numbers*/',offset=s.text.indexOf(annotation)+annotation.length;const selected=c.getTypeOfSymbol(c.getAliasedSymbol(c.getSymbolAtLocation(sem.getTokenAtPosition(s,offset))));const summary=c.typeToString(selected);
 // Explicit later whole-file inventory: this is a different checking history,
 // used only to pin native cause messages/spans, never product admission.
 const diagnostics=[...p.program.getGlobalDiagnostics(),...p.program.getSemanticDiagnostics()].filter(d=>[2316,2317,2318].includes(d.code)&&d.text.includes("'Array'")).map(d=>{const source=d.fileName?p.program.getSourceFile(d.fileName):null;return{code:d.code,message:d.text,...(source?{location:{file:path.relative(directory,d.fileName),start:position(lineStarts(source.text),d.pos),end:position(lineStarts(source.text),d.end)}}:{})};});rows.push({case:name,question:'GetAliasedSymbol→GetTypeOfSymbol then already accumulated GetGlobalDiagnostics and later file GetSemanticDiagnostics',summary,diagnostics});
 }finally{snapshot?.dispose();api.close();}}
fs.writeFileSync('.cc/temp/json-native-global-array-cause-locations.json',JSON.stringify(rows,null,2)+'\n');console.log(JSON.stringify(rows));

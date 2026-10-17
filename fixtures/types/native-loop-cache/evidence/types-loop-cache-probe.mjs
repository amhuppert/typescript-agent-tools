import fs from 'node:fs';
import path from 'node:path';
import {typeOracle} from '../../tools/type-oracle.mjs';
import {loadSemanticApi,lineStarts,position} from '../../tools/semantic-oracle.mjs';
const root=path.resolve('fixtures/types/native-loop-cache'),source=fs.readFileSync(path.join(root,'loops.ts'),'utf8'),starts=lineStarts(source),rows=[];
const markers=[...source.matchAll(/\/\*#(\w+)\*\//g)].map(match=>match[1]);
const native=await loadSemanticApi();
for(const config of ['tsconfig.json','loose.json'])for(const marker of markers){
 const offset=source.indexOf(`/*#${marker}*/`)+marker.length+5;
 const request={at:{file:'loops.ts',...position(starts,offset)},diagnostics:false};
 const answer=await typeOracle(path.join(root,config),request);
 const api=new native.API({tsserverPath:native.binary,cwd:root});let snapshot,identity;
 try{
  snapshot=api.updateSnapshot({openProject:path.join(root,config)});
  const project=snapshot.getProject(path.join(root,config));
  const inputs=[project.program.getConfigFileParsingDiagnostics(),project.program.getProgramDiagnostics(),project.program.getSyntacticDiagnostics(),project.program.getBindDiagnostics()].map(rows=>rows.map(row=>row.code));
  const file=project.program.getSourceFile(path.join(root,'loops.ts')),node=native.getTokenAtPosition(file,offset),checker=project.checker;
  const actual=checker.getTypeAtLocation(node),raw=checker.getTypeOfSymbol(checker.getSymbolAtLocation(node));
  identity={inputs,rawId:raw.id,actualId:actual.id,sameIdentity:actual.id===raw.id,rawSummary:checker.typeToString(raw),actualSummary:checker.typeToString(actual),errorType:actual.isErrorType()};
 }finally{try{snapshot?.dispose();}finally{api.close();}}
 rows.push({config,marker,request,answer,identity});
 process.stdout.write(JSON.stringify({config,marker,summary:answer.summary,identity})+'\n');
 fs.writeFileSync('.cc/temp/types-loop-cache-api.json',JSON.stringify(rows,null,2)+'\n');
}

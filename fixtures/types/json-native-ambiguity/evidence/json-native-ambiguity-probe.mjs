import fs from 'node:fs';
import path from 'node:path';
import { loadSemanticApi, lineStarts, position } from '../../tools/semantic-oracle.mjs';
const root=path.resolve('fixtures/types/json-native-ambiguity'), config=path.join(root,'tsconfig.json');
const semantic=await loadSemanticApi(),rows=[];
for(const file of ['first.ts','second.ts']) {
 const api=new semantic.API({tsserverPath:semantic.binary,cwd:root});let snapshot;
 try {
  snapshot=api.updateSnapshot({openProject:config});const project=snapshot.getProject(config),c=project.checker;
  const source=project.program.getSourceFile(path.join(root,file)), marker='/*#collision*/',offset=source.text.indexOf(marker)+marker.length;
  const alias=c.getSymbolAtLocation(semantic.getTokenAtPosition(source,offset)),target=c.getAliasedSymbol(alias),type=c.getTypeOfSymbol(target);
  const origins=(target.declarations??[]).map(handle=>{const declaration=handle.resolve(project);let source=declaration;while(source.parent)source=source.parent;const name=declaration.name??declaration;return {name:target.name,file:path.relative(root,source.fileName),start:position(lineStarts(source.text),name.getStart(source)),end:position(lineStarts(source.text),name.end)};});
  rows.push({file,at:position(lineStarts(source.text),offset),question:'GetAliasedSymbol→GetTypeOfSymbol',summary:c.typeToString(type),errorType:type.isErrorType(),origins,preDemandDiagnostics:[...project.program.getConfigFileParsingDiagnostics(),...project.program.getProgramDiagnostics(),...project.program.getSyntacticDiagnostics(),...project.program.getBindDiagnostics()].map(d=>d.code)});
 }finally{snapshot?.dispose();api.close();}
}
fs.writeFileSync('.cc/temp/json-native-ambiguity-probe.json',JSON.stringify(rows,null,2)+'\n');console.log(JSON.stringify(rows));

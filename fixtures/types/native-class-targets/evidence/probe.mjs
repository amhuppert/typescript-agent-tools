import fs from 'node:fs';
import path from 'node:path';
import { loadSemanticApi } from '../../../tools/semantic-oracle.mjs';
const native=await loadSemanticApi();const rows=[];
for(const name of ['generic', 'nongeneric', 'merged-default', 'namespace-first', 'interface-first', 'escaped-first', 'unused-constraint', 'overridden-default']){
 const root=path.resolve('fixtures/types/native-class-targets',name),config=path.join(root,'tsconfig.json');
 const api=new native.API({tsserverPath:native.binary,cwd:root});let snapshot;
 try{
  snapshot=api.updateSnapshot({openProject:config}); const project=snapshot.getProject(config),c=project.checker;
  const inputDiagnostics=[...project.program.getConfigFileParsingDiagnostics(),...project.program.getProgramDiagnostics(),...project.program.getSyntacticDiagnostics(),...project.program.getBindDiagnostics()];
  const source=project.program.getSourceFile(path.join(root,'globals.ts')),offset=source.text.indexOf('/*#query*/')+'/*#query*/'.length;
  const token=native.getTokenAtPosition(source,offset);if(token.parent.kind!==native.SyntaxKind.TypeReference)throw new Error('Wrong question owner');
  const typ=c.getTypeAtLocation(token.parent);const arguments_=typ.flags===native.TypeFlags.Object?c.getTypeArguments(typ):[];
  rows.push({name,source:source.text,question:'GetTypeAtLocation(actual TypeReference)',nodeKind:token.parent.kind,inputDiagnostics,summary:c.typeToString(typ),flags:typ.flags,errorType:typ.isErrorType(),arguments:arguments_.map(t=>({summary:c.typeToString(t),flags:t.flags,errorType:t.isErrorType()}))});
 }finally{try{snapshot?.dispose();}finally{api.close();}}
}
fs.writeFileSync(new URL('answers.json',import.meta.url),JSON.stringify(rows,null,2)+'\n');
console.log(JSON.stringify(rows.map(({name,summary,flags,errorType,arguments:a,inputDiagnostics})=>({name,summary,flags,errorType,arguments:a,inputDiagnosticCount:inputDiagnostics.length}))));

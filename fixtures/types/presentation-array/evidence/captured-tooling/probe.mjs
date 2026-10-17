import fs from 'node:fs';
import path from 'node:path';
import { loadSemanticApi } from '../../tools/semantic-oracle.mjs';
const native = await loadSemanticApi();
const answers = [];
for (const family of ['merged','invalid-index']) for (const configName of ['tsconfig.json','loose.json']) {
  const directory = path.resolve('fixtures/types/presentation-array',family);
  const config = path.join(directory,configName);
  const api = new native.API({tsserverPath:native.binary,cwd:directory});
  let snapshot;
  try {
    snapshot = api.updateSnapshot({openProject:config});
    const project = snapshot.getProject(config);
    const inputDiagnostics = [project.program.getConfigFileParsingDiagnostics(),project.program.getProgramDiagnostics(),project.program.getSyntacticDiagnostics(),project.program.getBindDiagnostics()];
    const source = project.program.getSourceFile(path.join(directory,'consumer.ts'));
    const token = native.getTokenAtPosition(source,source.text.indexOf('*/values')+2);
    const type = project.checker.getTypeOfSymbol(project.checker.getSymbolAtLocation(token));
    const describe = value => ({summary:project.checker.typeToString(value),errorType:value.isErrorType(),id:value.id});
    const before = {...describe(type),isArray:project.checker.isArrayType(type),arguments:project.checker.getTypeArguments(type).map(describe)};
    const indexes = project.checker.getIndexInfosOfType(type).map(info => ({key:describe(info.keyType),value:describe(info.valueType),readonly:info.isReadonly}));
    answers.push({family,config:configName,inputDiagnostics,before,indexes,after:describe(type)});
  } finally {try {snapshot?.dispose();} finally {api.close();}}
}
fs.writeFileSync('.cc/temp/type-presentation-array-probe.json',JSON.stringify({version:native.version,answers},null,2)+'\n');
console.log(JSON.stringify({version:native.version,answers},null,2));

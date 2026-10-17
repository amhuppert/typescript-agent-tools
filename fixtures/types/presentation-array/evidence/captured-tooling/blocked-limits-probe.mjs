import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { loadSemanticApi } from '../../tools/semantic-oracle.mjs';
const native = await loadSemanticApi();
const root = path.resolve('.cc/temp/type-array-blocked-limits');
const answers = [];
for (const literalBytes of [260866,260867,260868]) for (const strict of [false,true]) {
  const directory = path.join(root,`${literalBytes}-${strict}`);
  fs.mkdirSync(directory,{recursive:true});
  const files = {'globals.ts':'interface Array<T> {}\ninterface Array<T> {}\n',
    'value.ts':`export let current: Array<"${'p'.repeat(literalBytes)}">;\n`,
    'consumer.ts':"import { current } from './value';\n",
    'tsconfig.json':JSON.stringify({compilerOptions:{strict,noLib:true},files:['globals.ts','consumer.ts']})+'\n'};
  for (const [file,text] of Object.entries(files)) fs.writeFileSync(path.join(directory,file),text);
  const config = path.join(directory,'tsconfig.json');
  const api = new native.API({tsserverPath:native.binary,cwd:directory});
  let snapshot;
  try {
    snapshot = api.updateSnapshot({openProject:config});
    const project = snapshot.getProject(config);
    const inputDiagnostics = [project.program.getConfigFileParsingDiagnostics(),project.program.getProgramDiagnostics(),project.program.getSyntacticDiagnostics(),project.program.getBindDiagnostics()];
    const source = project.program.getSourceFile(path.join(directory,'consumer.ts'));
    const token = native.getTokenAtPosition(source,source.text.indexOf('current'));
    const type = project.checker.getTypeOfSymbol(project.checker.getSymbolAtLocation(token));
    const args = project.checker.getTypeArguments(type);
    const argumentSummary = project.checker.typeToString(args[0]);
    answers.push({literalBytes,strict,inputDiagnostics,root:{summary:project.checker.typeToString(type),errorType:type.isErrorType(),isArray:project.checker.isArrayType(type)},
      argument:{count:args.length,flags:args[0].flags,errorType:args[0].isErrorType(),summaryBytes:argumentSummary.length,summary:argumentSummary},
      members:{properties:project.checker.getPropertiesOfType(type).map(symbol=>symbol.name),
        calls:project.checker.getSignaturesOfType(type,native.SignatureKind.Call).length,
        constructs:project.checker.getSignaturesOfType(type,native.SignatureKind.Construct).length,
        indexes:project.checker.getIndexInfosOfType(type).length},
      inputHashes:Object.entries(files).map(([file,text])=>({file,sha256:createHash('sha256').update(text).digest('hex')}))});
  } finally {try {snapshot?.dispose();} finally {api.close();}}
}
fs.writeFileSync(path.join(root,'observations.json'),JSON.stringify({version:native.version,answers},null,2)+'\n');
console.log(JSON.stringify(answers,null,2));

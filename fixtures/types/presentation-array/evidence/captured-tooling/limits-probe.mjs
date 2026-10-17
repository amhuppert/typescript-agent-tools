import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { loadSemanticApi } from '../../tools/semantic-oracle.mjs';
const native = await loadSemanticApi();
const root = path.resolve('.cc/temp/type-array-limits');
const answers = [];
const families = [
  ...[1996,1997,1998].map(count => ({id:`nodes-${count}`,count,globals:'interface Array<T> { [index: number]: T; }\n',
    value:`export let current: Array<${Array.from({length:count},(_,i)=>i+1).join(' | ')}>;\n`})),
  ...[261225,261226,261227].map(keyBytes => ({id:`bytes-${keyBytes}`,keyBytes,
    globals:`interface Array<T> { ${'p'.repeat(keyBytes)}: string extends number ? string : number; }\n`,
    value:'export let current: Array<number>;\n'})),
  {id:'metadata-invalid',keyBytes:263000,globals:`interface Array<T> { ${'p'.repeat(263000)}: MissingType; }\n`,
    value:'export let current: Array<number>;\n'},
];
for (const family of families) for (const strict of [false,true]) {
  const directory = path.join(root,`${family.id}-${strict}`);
  fs.mkdirSync(directory,{recursive:true});
  const files = {'globals.ts':family.globals,'value.ts':family.value,'consumer.ts':"import { current } from './value';\n",
    'tsconfig.json':JSON.stringify({compilerOptions:{strict,noLib:true},files:['globals.ts','consumer.ts']})+'\n'};
  for (const [name,text] of Object.entries(files)) fs.writeFileSync(path.join(directory,name),text);
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
    const describe = value => ({summary:project.checker.typeToString(value),errorType:value.isErrorType()});
    const answer = {id:family.id,strict,inputDiagnostics,root:describe(type),isArray:project.checker.isArrayType(type),
      argumentCount:args.length,argumentError:args[0].isErrorType(),
      inputHashes:Object.entries(files).map(([file,text])=>({file,sha256:createHash('sha256').update(text).digest('hex')}))};
    if (family.count) answer.constituents = args[0].getTypes().length;
    else {
      const properties = project.checker.getPropertiesOfType(type);
      answer.propertyKeyBytes = properties[0].name.length;
      answer.property = describe(project.checker.getTypeOfSymbol(properties[0]));
      const globalSource = project.program.getSourceFile(path.join(directory,'globals.ts'));
      const declaration = properties[0].declarations[0].resolve(project);
      answer.annotationSpan = {start:declaration.type.getStart(globalSource)+1,end:declaration.type.end+1};
    }
    answers.push(answer);
  } finally {try {snapshot?.dispose();} finally {api.close();}}
}
fs.writeFileSync(path.join(root,'observations.json'),JSON.stringify({version:native.version,answers},null,2)+'\n');
console.log(JSON.stringify(answers.map(({root,...row})=>({...row,root:{errorType:root.errorType,summaryBytes:root.summary.length}})),null,2));

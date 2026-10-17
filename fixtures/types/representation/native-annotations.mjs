// Independent expected results for direct primitive annotation type sites.
// This is a compiler experiment, not the full T fresh-request baseline.
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {loadSemanticApi} from '../../../tools/semantic-oracle.mjs';
const directory=path.resolve('.cc/temp/types-representation');
const manifest=JSON.parse(fs.readFileSync(path.join(directory,'capture-manifest.json'),'utf8'));
const sources=new Map(manifest.sources.map(source=>[source.file,source]));
const rows=fs.readFileSync(path.join(directory,'time.tsv'),'utf8').trim().split('\n').filter(line=>line.startsWith('oracle\t')).map(line=>{
 const [,file,declaration,annotation,start,expected]=line.split('\t');
 return {file,declaration:Number(declaration),annotation:Number(annotation),startByte:Number(start),expected};
});
const config=path.join(directory,'tsconfig.json');
fs.writeFileSync(config,JSON.stringify({compilerOptions:{strict:true,noEmit:true},files:manifest.sources.filter(source=>source.control).map(source=>source.captured)},null,2)+'\n');
const semantic=await loadSemanticApi();
const answers=[];
for(const [group,configuration] of [['corpus','/Users/alex/github/command-center/tsconfig.json'],['control',config]]) {
 const api=new semantic.API({tsserverPath:semantic.binary,cwd:path.dirname(configuration)});
 let snapshot;
 try {
  snapshot=api.updateSnapshot({openProject:configuration});
  const project=snapshot.getProject(configuration);if(!project)throw Error('Missing native project');
  const failures=[...project.program.getConfigFileParsingDiagnostics(),...project.program.getProgramDiagnostics(),...project.program.getSyntacticDiagnostics(),...project.program.getBindDiagnostics()].filter(d=>d.category===1);
  if(failures.length)throw Error(JSON.stringify(failures));
  for(const row of rows) {
   const input=sources.get(row.file);if((input.control?'control':'corpus')!==group)continue;
   const original=input.original;
   const source=project.program.getSourceFile(original);if(!source)throw Error('Missing native source '+original);
   const raw=fs.readFileSync(input.captured);
   const hash=createHash('sha256').update(raw).digest('hex');
   if(hash!==input.sha256||!fs.readFileSync(original).equals(raw))throw Error('Original/captured source differs '+row.file);
   const capturedText=raw.toString('utf8');
   // This probe records the native reader's observed one-BOM normalization.
   // Accept no other difference and translate only the resulting UTF-16 offset.
   const nativeRemovedBOM=capturedText.startsWith('\ufeff')&&source.text===capturedText.slice(1);
   if(source.text!==capturedText&&!nativeRemovedBOM)throw Error('Native/captured source differs beyond leading BOM '+row.file);
   const nativeSourceHash=createHash('sha256').update(source.text).digest('hex');
   const offset=raw.subarray(0,row.startByte).toString('utf8').length-Number(nativeRemovedBOM);
   const token=semantic.getTokenAtPosition(source,offset);
   if(!semantic.isTypeNode(token))throw Error('Annotation anchor did not select a type node');
   const type=project.checker.getTypeAtLocation(token);if(!type)throw Error('Native returned no type');
   const summary=project.checker.typeToString(type),errorType=type.isErrorType();
   if(summary!==row.expected||errorType)throw Error(JSON.stringify({row,summary,errorType}));
   answers.push({...row,sourceHash:hash,nativeSourceHash,nativeRemovedBOM,offsetUTF16:offset,syntax:semantic.SyntaxKind[token.kind],summary,errorType,compiler:semantic.version});
  }
 } finally {snapshot?.dispose();api.close();}
}
fs.writeFileSync(path.join(directory,'native-annotations.json'),JSON.stringify({compiler:semantic.version,questions:answers,limits:'Two native project/checker sessions; direct primitive type nodes only, no binding, overload/prelude, factory, flow or member conformance claim'},null,2)+'\n');
console.log(JSON.stringify({compiler:semantic.version,primitiveAnnotationComparisons:answers.length,mismatches:0}));

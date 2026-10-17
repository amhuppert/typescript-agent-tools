// Independent raw TypeScript 7.0.2 alias-value controls; fresh checker per row.
import fs from 'node:fs';
import path from 'node:path';
import { loadSemanticApi, lineStarts, position } from '../../tools/semantic-oracle.mjs';
const root = path.resolve('fixtures/types/json-native-basic');
const semantic = await loadSemanticApi();
const text = fs.readFileSync(path.join(root, 'consumer.ts'), 'utf8');
const markers = ['stringValue','numberValue','booleanValue','duplicateValue','decodedValue','emptyValue','canonicalNumber','noncanonicalNumber','astralValue','nestedValue','typeOnlyValue'];
const rows = [];
for (const strict of [0,1]) for (const exact of [0,1]) for (const unchecked of [0,1]) {
  const config = path.join(root, `strict-${strict}-exact-${exact}-unchecked-${unchecked}.json`);
  for (const marker of markers) {
    const api = new semantic.API({tsserverPath:semantic.binary,cwd:root}); let snapshot;
    try {
      snapshot = api.updateSnapshot({openProject:config});
      const project = snapshot.getProject(config), checker = project.checker;
      const source = project.program.getSourceFile(path.join(root,'consumer.ts'));
      const offset = text.indexOf(`/*#${marker}*/`) + `/*#${marker}*/`.length;
      const token = semantic.getTokenAtPosition(source,offset);
      const alias = checker.getSymbolAtLocation(token), symbol = checker.getAliasedSymbol(alias);
      const type = checker.getTypeOfSymbol(symbol);
      const origins = symbol => (symbol.declarations ?? []).map(handle => {
        const declaration = handle.resolve(project); let source = declaration; while(source.parent) source=source.parent;
        const node = declaration.name ?? declaration;
        return {kind:semantic.SyntaxKind[declaration.kind],file:path.relative(root,source.fileName),start:position(lineStarts(source.text),node.getStart(source)),end:position(lineStarts(source.text),node.end)};
      });
      const properties = type => checker.getPropertiesOfType(type).map(property => {
        const child = checker.getTypeOfSymbol(property);
        return {name:property.name,summary:checker.typeToString(child),origins:origins(property),children:child.flags & semantic.TypeFlags.Object ? properties(child) : []};
      });
      rows.push({strict,exact,unchecked,marker,question:'GetAliasedSymbol→GetTypeOfSymbol',name:symbol.name,summary:checker.typeToString(type),errorType:type.isErrorType(),origins:origins(symbol),properties:marker==='nestedValue'?properties(type):[],fatalDiagnostics:[...project.program.getConfigFileParsingDiagnostics(),...project.program.getProgramDiagnostics(),...project.program.getSyntacticDiagnostics(),...project.program.getBindDiagnostics()].map(d=>d.code)});
    } finally {snapshot?.dispose();api.close();}
  }
}
fs.writeFileSync('.cc/temp/json-native-basic-alias-probe.json',JSON.stringify(rows,null,2)+'\n');
console.log(JSON.stringify({requests:rows.length,accepted:rows.filter(r=>r.fatalDiagnostics.length===0).length,invalid:rows.filter(r=>r.fatalDiagnostics.length>0).length,healthy:rows.filter(r=>r.strict===1&&r.exact===0&&r.unchecked===0)}));

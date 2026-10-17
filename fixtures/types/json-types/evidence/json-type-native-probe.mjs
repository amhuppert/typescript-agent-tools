// Throwaway expectation probe: raw pinned API operations, no oracle selection.
import fs from 'node:fs';
import path from 'node:path';
import { loadSemanticApi } from '../../tools/semantic-oracle.mjs';
const root = path.resolve('fixtures/types/json-types');
const sourceText = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const semantic = await loadSemanticApi();
const result = [];
for (const marker of ['root', 'text', 'number', 'boolean', 'nil', 'empty', 'numbers', 'numbers0', 'empty0', 'mixed', 'mixed0', 'shapes', 'shape0', 'shapeA', 'duplicate', 'nestedDuplicate', 'escaped', 'emptyKey', 'numericCanonical', 'numericNonCanonical', 'astral', 'defaultKey', 'namespaceWhole', 'syntheticCopy', 'namedCopy', 'missing', 'unknown', 'unknownRead', 'genuineAny', 'recursive', 'rootNull', 'rootEmpty', 'rootNumber', 'importRoot', 'syntheticDefault', 'namedDup']) {
  const config = path.join(root, 'tsconfig.json');
  const api = new semantic.API({ tsserverPath: semantic.binary, cwd: root });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config), checker = project.checker;
    const source = project.program.getSourceFile(path.join(root, 'cases.ts'));
    const offset = sourceText.indexOf(`/*#${marker}*/`) + `/*#${marker}*/`.length;
    const node = semantic.getTokenAtPosition(source, offset);
    const raw = checker.getSymbolAtLocation(node);
    const symbol = raw.flags & semantic.SymbolFlags.Alias ? checker.getAliasedSymbol(raw) : raw;
    const type = checker.getTypeOfSymbol(symbol);
    result.push({ marker, nodeKind: semantic.SyntaxKind[node.kind], name: symbol.name, followedAlias: symbol !== raw,
      fatalDiagnostics: [...project.program.getConfigFileParsingDiagnostics(), ...project.program.getProgramDiagnostics(),
        ...project.program.getSyntacticDiagnostics(), ...project.program.getBindDiagnostics()].map(d => d.code),
      summary: checker.typeToString(type), typeFlags: type.flags, errorType: type.isErrorType(),
      properties: ['root', 'namespaceWhole', 'syntheticCopy', 'nestedDuplicate'].includes(marker) ? checker.getPropertiesOfType(type).map(p => ({ name: p.name, summary: checker.typeToString(checker.getTypeOfSymbol(p)) })) : [] });
  } finally { snapshot?.dispose(); api.close(); }
}
fs.writeFileSync('.cc/temp/json-type-native-probe.json', JSON.stringify(result, null, 2));
console.log(JSON.stringify(result.map(({marker,summary,errorType}) => ({marker,summary,errorType})), null, 2));

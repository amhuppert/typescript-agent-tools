import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, loadSemanticApi, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-private-names/', import.meta.url));
test('private-name keys retain actual class identity and declaration origins', async () => {
  const file = 'cases.ts';
  const source = fs.readFileSync(path.join(root, file), 'utf8');
  const config = path.join(root, 'tsconfig.json');
  const rows = [];
  for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
    const at = {file, ...position(lineStarts(source), marker.index + marker[0].length)};
    const native = await compilerHookOracle(config, {at});
    const api = await typeOracle(config, {at, properties: false});
    assert.equal(native.compiler, '7.0.2');
    assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
    rows.push({name: marker[1], summary: native.summary, errorType: native.errorType});
  }
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  const api = new native.API({tsserverPath: native.binary, cwd: root});
  let snapshot;
  const inventories = [];
  try {
    snapshot = api.updateSnapshot({openProject: config});
    const project = snapshot.getProject(config);
    const fileNode = project.program.getSourceFile(path.join(root, file));
    const checker = project.checker;
    const classes = [];
    const visit = node => {
      if ([native.SyntaxKind.ClassDeclaration, native.SyntaxKind.ClassExpression].includes(node.kind)
          && node.name) classes.push(node);
      node.forEachChild(visit);
    };
    visit(fileNode);
    for (const declaration of classes) {
      const symbol = checker.getSymbolAtLocation(declaration.name);
      const types = [{name: declaration.name.text, type: checker.getDeclaredTypeOfSymbol(symbol)}];
      if (declaration.name.text === 'Base') {
        types.push({name: 'typeof Base', type: checker.getTypeOfSymbol(symbol)});
      }
      for (const {name: inventoryName, type} of types) {
        const properties = checker.getPropertiesOfType(type).map(property => {
          const declarations = (property.declarations ?? []).map(handle => {
            const own = handle.resolve(project);
            assert.ok(own, 'native declaration handle resolves in its project');
            return own;
          });
          const origins = declarations.map(own => {
            const start = own.name?.getStart(fileNode) ?? own.getStart(fileNode);
            return {...position(lineStarts(source), start), end: position(lineStarts(source), own.name?.end ?? own.end)};
          });
          const name = declarations[0]?.name;
          const privateName = name?.kind === native.SyntaxKind.PrivateIdentifier;
          const type = checker.getTypeOfSymbol(property);
          const typeSymbol = type.getSymbol();
          const nominal = typeSymbol && (typeSymbol.flags & (native.SymbolFlags.Class
            | native.SymbolFlags.Enum | native.SymbolFlags.ValueModule | native.SymbolFlags.Function));
          const handle = nominal ? typeSymbol.declarations?.[0] : undefined;
          let graphOrigin = null;
          if (handle) {
            const own = handle.resolve(project);
            assert.ok(own, 'nominal type declaration resolves');
            graphOrigin = {file, ...position(lineStarts(source), (own.name ?? own).getStart(fileNode))};
          }
          return {kind: privateName ? 'private' : 'string', name: privateName ? name.text : property.name,
            summary: checker.typeToString(type), origins, graphOrigin};
        });
        // Numeric native symbol IDs are deliberately not identity or order in
        // this product. Sort captured records by exact key and actual source.
        const compare = (a, b) => a < b ? -1 : a > b ? 1 : 0;
        properties.sort((a, b) => compare(a.kind, b.kind) || compare(a.name, b.name)
          || (a.origins[0]?.line ?? 0) - (b.origins[0]?.line ?? 0)
          || (a.origins[0]?.column ?? 0) - (b.origins[0]?.column ?? 0));
        inventories.push({name: inventoryName, properties});
      }
    }
  } finally {
    try { snapshot?.dispose(); } finally { api.close(); }
  }
  const result = {rows, inventories};
  fs.mkdirSync('.cc/temp', {recursive: true});
  fs.writeFileSync('.cc/temp/native-private-names.json', JSON.stringify(result, null, 2) + '\n');
  assert.equal(rows.length, 8);
  assert.deepEqual(result, JSON.parse(fs.readFileSync(path.join(root, 'expected.json'), 'utf8')));
});

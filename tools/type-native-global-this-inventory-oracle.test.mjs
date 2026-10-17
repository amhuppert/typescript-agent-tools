import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {lineStarts, loadSemanticApi, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-global-this/', import.meta.url));

test('native globalThis merged symbol inventories and signatures', async () => {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  const rows = [];
  for (const scenario of ['clean', 'variable', 'block', 'function', 'namespace', 'interface', 'class', 'enum', 'ambient']) {
    const api = new native.API({tsserverPath: native.binary, cwd: root});
    let snapshot;
    try {
      const config = path.join(root, `${scenario}.json`);
      snapshot = api.updateSnapshot({openProject: config});
      const project = snapshot.getProject(config);
      const source = project.program.getSourceFile(path.join(root, `${scenario}.ts`));
      const declaration = source.statements.flatMap(statement => statement.declarationList?.declarations ?? [])
        .find(declaration => declaration.name.text === 'self');
      const checker = project.checker;
      const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(declaration.name));
      const symbol = type.getSymbol();
      // The product qualifies nominal graph labels from their actual first
      // declaration; capture that independently of its renderer. Top-level
      // compiler summaries retain their ordinary spelling.
      const describeType = type => {
        const summary = checker.typeToString(type);
        const symbol = type.getSymbol();
        const nominal = symbol && (symbol.flags & (native.SymbolFlags.Class
          | native.SymbolFlags.Enum | native.SymbolFlags.ValueModule | native.SymbolFlags.Function));
        const handle = nominal ? symbol.declarations?.[0] : undefined;
        let graphOrigin = null;
        if (handle) {
          const own = handle.resolve(project);
          assert.ok(own, 'nominal declaration resolves in its native project');
          const token = own.name ?? own;
          const text = fs.readFileSync(path.join(root, `${scenario}.ts`), 'utf8');
          graphOrigin = {file: `${scenario}.ts`, ...position(lineStarts(text), token.getStart(source))};
        }
        return {summary, errorType: type.isErrorType(), graphOrigin};
      };
      const properties = checker.getPropertiesOfType(type).map(property => ({
        name: property.name, ...describeType(checker.getTypeOfSymbol(property)),
      })).sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
      const describeSignatures = kind => checker.getSignaturesOfType(type, kind).map(signature => ({
        parameters: signature.getParameters().map(parameter => checker.typeToString(checker.getTypeOfSymbol(parameter))),
        returns: describeType(checker.getReturnTypeOfSignature(signature)),
      }));
      rows.push({scenario, summary: checker.typeToString(type), symbolFlags: symbol.flags,
        declarationCount: symbol.declarations.length, properties,
        calls: describeSignatures(native.SignatureKind.Call),
        constructors: describeSignatures(native.SignatureKind.Construct),
        indexes: checker.getIndexInfosOfType(type).map(index => ({
          key: checker.typeToString(index.keyType), value: checker.typeToString(index.valueType),
        }))});
    } finally {
      try { snapshot?.dispose(); } finally { api.close(); }
    }
  }
  fs.mkdirSync('.cc/temp', {recursive: true});
  fs.writeFileSync('.cc/temp/native-global-this-inventory.json', JSON.stringify(rows, null, 2) + '\n');
  assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, 'expected-inventory.json'), 'utf8')));
});

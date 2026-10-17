import fs from 'node:fs';
import path from 'node:path';
import { loadSemanticApi } from '../../tools/semantic-oracle.mjs';

// Research control only: each operation gets a fresh checker. Input diagnostics
// precede every semantic question; no whole-file semantic pass is requested.
const root = path.resolve('fixtures/types/json-generic-targets');
const native = await loadSemanticApi();
const rows = [];
const cases = JSON.parse(fs.readFileSync(path.join(root, 'cases.json'), 'utf8'));
for (const name of cases) {
  const operations = ['identity', 'inventory', 'value', 'constraint', 'default'];
  if (name === 'unused-member') operations.push('broken');
  if (name === 'basic' || name === 'valid-class') operations.push('self', 'first');
  for (const operation of operations) {
    const directory = path.join(root, name);
    const config = path.join(directory, 'strict-1-exact-0-unchecked-0.json');
    const api = new native.API({ tsserverPath: native.binary, cwd: directory });
    let snapshot;
    try {
      snapshot = api.updateSnapshot({ openProject: config });
      const project = snapshot.getProject(config);
      if (!project) throw new Error(`unopened project ${config}`);
      const c = project.checker;
      const preDemandDiagnostics = [...project.program.getConfigFileParsingDiagnostics(), ...project.program.getProgramDiagnostics(), ...project.program.getSyntacticDiagnostics(), ...project.program.getBindDiagnostics()].map(d => ({ code: d.code, text: d.text }));
      const source = project.program.getSourceFile(path.join(directory, 'consumer.ts'));
      const marker = '/*#numbers*/';
      const node = native.getTokenAtPosition(source, source.text.indexOf(marker) + marker.length);
      const alias = c.getSymbolAtLocation(node);
      const valueSymbol = c.getAliasedSymbol(alias);
      const type = c.getTypeOfSymbol(valueSymbol);
      const target = type.getTarget();
      const parameters = target.getTypeParameters();
      const describe = type => type && ({ id: type.id, summary: c.typeToString(type), flags: type.flags, objectFlags: type.objectFlags, errorType: type.isErrorType(), isThisType: type.isThisType });
      const row = {
        case: name, operation, question: 'GetAliasedSymbol→GetTypeOfSymbol→target identities',
        preDemandDiagnostics,
        sourceFiles: project.program.getSourceFileNames().map(file => path.relative(directory, file)).sort(),
        root: describe(type), target: describe(target), isArray: c.isArrayType(type),
        typeArguments: c.getTypeArguments(type).map(describe), parameters: parameters.map(describe),
        rootSymbolName: type.getSymbol()?.name ?? null,
      };
      if (operation === 'inventory') {
        row.properties = c.getPropertiesOfType(type).map(symbol => symbol.name);
        row.indexes = c.getIndexInfosOfType(type).map(index => ({ key: describe(index.keyType), value: describe(index.valueType), readonly: index.isReadonly }));
        row.callCount = c.getSignaturesOfType(type, native.SignatureKind.Call).length;
        row.constructCount = c.getSignaturesOfType(type, native.SignatureKind.Construct).length;
      } else if (operation === 'constraint') {
        row.constraint = describe(c.getConstraintOfTypeParameter(parameters[0])) ?? null;
      } else if (operation === 'default') {
        const globals = project.program.getSourceFile(path.join(directory, 'globals.ts'));
        row.parameterDeclarations = globals.statements.filter(statement => statement.name?.text === 'Array').map(statement => {
          const declaration = statement.typeParameters[0];
          const symbol = c.getSymbolAtLocation(declaration.name);
          return {
            canonicalParameter: describe(c.getDeclaredTypeOfSymbol(symbol)),
            hasConstraint: !!declaration.constraint, hasDefault: !!declaration.defaultType,
            defaultTypeNodeQuestion: declaration.defaultType ? describe(c.getTypeAtLocation(declaration.defaultType)) : null,
          };
        });
      } else if (operation !== 'identity') {
        const symbol = c.getPropertyOfType(type, operation);
        if (!symbol) throw new Error(`missing expected ${operation} property in ${name}`);
        const memberType = c.getTypeOfSymbol(symbol);
        row.member = { name: symbol.name, type: describe(memberType) };
        if (operation === 'first') row.member.calls = c.getSignaturesOfType(memberType, native.SignatureKind.Call).map(signature => ({ parameters: signature.getParameters().map(symbol => describe(c.getTypeOfSymbol(symbol))), returns: describe(c.getReturnTypeOfSignature(signature)) }));
      }
      rows.push(row);
      console.log(JSON.stringify({ case: name, operation, root: row.root.summary, parameters: row.parameters.map(x => x.summary), result: row.constraint ?? row.member ?? row.properties ?? row.parameterDeclarations ?? null, inputCodes: preDemandDiagnostics.map(x => x.code) }));
    } finally {
      try { snapshot?.dispose(); } finally { api.close(); }
    }
  }
}
fs.writeFileSync('.cc/temp/json-native-generic-array-probe.json', JSON.stringify(rows, null, 2) + '\n');

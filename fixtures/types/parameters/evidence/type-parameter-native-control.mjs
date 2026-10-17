import fs from 'node:fs';
import path from 'node:path';
import { loadSemanticApi } from '../../tools/semantic-oracle.mjs';
const native = await loadSemanticApi();
const root = path.resolve('.cc/temp/type-parameter-native-input');
fs.mkdirSync(root, { recursive: true });
const sources = {
  any: 'interface Sample<T extends any> { value: T; }\n',
  unknown: 'interface Sample<T extends unknown> { value: T; }\n',
  default: 'interface Sample<T = "ready"> { value: T; }\n',
  missingConstraint: 'interface Sample<T extends MissingConstraint> { value: T; }\n',
  missingDefault: 'interface Sample<T = MissingDefault> { value: T; }\n',
};
const rows = [];
for (const [name, source] of Object.entries(sources)) {
  const directory = path.join(root, name); fs.mkdirSync(directory, { recursive: true });
  fs.writeFileSync(path.join(directory, 'globals.ts'), source);
  const config = path.join(directory, 'tsconfig.json');
  fs.writeFileSync(config, JSON.stringify({ compilerOptions: { strict: true, noLib: true }, files: ['globals.ts'] }));
  for (const operation of ['root', 'constraint', 'defaultNode']) {
    const api = new native.API({ tsserverPath: native.binary, cwd: directory }); let snapshot;
    try {
      snapshot = api.updateSnapshot({ openProject: config }); const project = snapshot.getProject(config); const c = project.checker;
      const input = [...project.program.getConfigFileParsingDiagnostics(), ...project.program.getProgramDiagnostics(), ...project.program.getSyntacticDiagnostics(), ...project.program.getBindDiagnostics()];
      if (input.length) throw new Error(JSON.stringify(input));
      const file = project.program.getSourceFile(path.join(directory, 'globals.ts'));
      const declaration = file.statements[0].typeParameters[0];
      const reference = native.getTokenAtPosition(file, source.indexOf('value: T') + 'value: '.length);
      // Product type-reference selection asks the TypeReference owner node;
      // GetTypeAtLocation on its bare Identifier is a different native question.
      const root = c.getTypeAtLocation(reference.parent);
      const describe = type => type && ({ id: type.id, summary: c.typeToString(type), flags: type.flags, errorType: type.isErrorType() });
      const row = { name, operation, inputCodes: input.map(d => d.code), root: describe(root), canonicalParameter: describe(c.getDeclaredTypeOfSymbol(c.getSymbolAtLocation(declaration.name))) };
      if (operation === 'constraint') row.constraint = describe(c.getConstraintOfTypeParameter(root)) ?? null;
      if (operation === 'defaultNode') row.defaultNode = declaration.defaultType ? describe(c.getTypeAtLocation(declaration.defaultType)) : null;
      rows.push(row);
    } finally { try { snapshot?.dispose(); } finally { api.close(); } }
  }
}
fs.writeFileSync('.cc/temp/type-parameter-native-control.json', JSON.stringify(rows, null, 2) + '\n');
console.log(JSON.stringify(rows));

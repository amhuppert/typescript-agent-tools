// Raw pinned API baseline. Every row opens and disposes a fresh checker.
import fs from 'node:fs';
import path from 'node:path';
import { loadSemanticApi, lineStarts, position } from '../../tools/semantic-oracle.mjs';
const root = path.resolve('fixtures/types/json-types');
const semantic = await loadSemanticApi();
const sourceText = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const rows = [];
for (const strict of [0, 1]) for (const exact of [0, 1]) for (const unchecked of [0, 1]) {
  for (const marker of ['nil', 'empty', 'empty0', 'numbers0', 'mixed0', 'shapes', 'shapeA', 'rootNull', 'rootEmpty']) {
    const config = path.join(root, `strict-${strict}-exact-${exact}-unchecked-${unchecked}.json`);
    const api = new semantic.API({ tsserverPath: semantic.binary, cwd: root });
    let snapshot;
    try {
      snapshot = api.updateSnapshot({ openProject: config });
      const project = snapshot.getProject(config), checker = project.checker;
      const source = project.program.getSourceFile(path.join(root, 'cases.ts'));
      const offset = sourceText.indexOf(`/*#${marker}*/`) + `/*#${marker}*/`.length;
      const token = semantic.getTokenAtPosition(source, offset);
      const type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(token));
      rows.push({ strict, exact, unchecked, marker, summary: checker.typeToString(type), errorType: type.isErrorType(),
        programDiagnostics: project.program.getProgramDiagnostics().map(d => d.code) });
    } finally { snapshot?.dispose(); api.close(); }
  }
}
const keys = [];
const keyText = fs.readFileSync(path.join(root, 'data.json'), 'utf8');
for (const [spelling, nth] of [['"dup"', 0], ['"dup"', 1], ['"dup"', 2], ['"dup"', 3], ['"a"', 1], ['"\\u0061"', 0], ['""', 0], ['"01"', 0], ['"1"', 0], ['"😀"', 0]]) {
  const api = new semantic.API({ tsserverPath: semantic.binary, cwd: root });
  let snapshot;
  try {
    const config = path.join(root, 'tsconfig.json');
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config), checker = project.checker;
    const source = project.program.getSourceFile(path.join(root, 'data.json'));
    let offset = -1;
    for (let index = 0; index <= nth; index++) offset = keyText.indexOf(spelling, offset + 1);
    if (offset < 0) throw new Error(`missing ${spelling} ${nth}`);
    const token = semantic.getTokenAtPosition(source, offset);
    const symbol = checker.getSymbolAtLocation(token);
    const origins = (symbol.declarations ?? []).map(handle => {
      const declaration = handle.resolve(project), name = declaration.name ?? declaration;
      return { kind: semantic.SyntaxKind[declaration.kind], start: position(lineStarts(keyText), name.getStart(source)),
        end: position(lineStarts(keyText), name.end), raw: keyText.slice(name.getStart(source), name.end) };
    });
    const type = checker.getTypeOfSymbol(symbol);
    keys.push({ spelling, nth, name: symbol.name, symbolFlags: symbol.flags, summary: checker.typeToString(type), errorType: type.isErrorType(), origins });
  } finally { snapshot?.dispose(); api.close(); }
}
const uses = [];
for (const marker of ['memberDup', 'memberEscaped', 'memberEmpty', 'memberNumber']) {
  const api = new semantic.API({ tsserverPath: semantic.binary, cwd: root });
  let snapshot;
  try {
    const config = path.join(root, 'tsconfig.json');
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config), checker = project.checker;
    const source = project.program.getSourceFile(path.join(root, 'cases.ts'));
    const offset = sourceText.indexOf(`/*#${marker}*/`) + `/*#${marker}*/`.length;
    const token = semantic.getTokenAtPosition(source, offset), symbol = checker.getSymbolAtLocation(token);
    uses.push({ marker, name: symbol.name, symbolFlags: symbol.flags });
  } finally { snapshot?.dispose(); api.close(); }
}
const recoveryControl = [];
for (const unchecked of [0, 1]) {
  const api = new semantic.API({ tsserverPath: semantic.binary, cwd: root });
  let snapshot;
  try {
    const config = path.join(root, `strict-1-exact-0-unchecked-${unchecked}.json`);
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config), checker = project.checker;
    const source = project.program.getSourceFile(path.join(root, 'cases.ts'));
    const offset = sourceText.indexOf('/*#shapeA*/') + '/*#shapeA*/'.length;
    const token = semantic.getTokenAtPosition(source, offset), type = checker.getTypeOfSymbol(checker.getSymbolAtLocation(token));
    recoveryControl.push({ unchecked, summary: checker.typeToString(type), errorType: type.isErrorType(),
      diagnostics: project.program.getSemanticDiagnostics().filter(d => d.fileName === source.fileName && position(lineStarts(sourceText), d.pos).line === 17).map(d => d.code) });
  } finally { snapshot?.dispose(); api.close(); }
}
fs.writeFileSync('.cc/temp/json-options-native-probe.json', JSON.stringify({ rows, keys, uses, recoveryControl }, null, 2));
console.log(JSON.stringify({ rows, keys, uses, recoveryControl }, null, 2));

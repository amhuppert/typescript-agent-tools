import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { loadSemanticApi } from '../../../tools/semantic-oracle.mjs';
const directory = path.dirname(fileURLToPath(import.meta.url));
const semantic = await loadSemanticApi();
if (semantic.version !== '7.0.2') throw new Error('wrong pinned version');
const rows = [];
for (const strict of [true, false]) for (const length of [261394, 261395, 261396, 263000]) {
  const root = path.join(directory, `${strict}-${length}`);
  fs.mkdirSync(root, { recursive: true });
  const key = 'p'.repeat(length);
  const annotation = length === 263000 ? 'MissingType' : 'string extends number ? string : number';
  const source = `export interface Budget { ${key}: ${annotation}; }\n`;
  fs.writeFileSync(path.join(root, 'value.ts'), source);
  fs.writeFileSync(path.join(root, 'tsconfig.json'), JSON.stringify({ compilerOptions: { noLib: true, strict, noEmit: true }, files: ['value.ts'] }));
  const config = path.join(root, 'tsconfig.json');
  const api = new semantic.API({ tsserverPath: semantic.binary, cwd: root });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config);
    if (!project) throw new Error('project missing');
    const program = project.program;
    const inputDiagnostics = [...program.getConfigFileParsingDiagnostics(), ...program.getProgramDiagnostics(), ...program.getSyntacticDiagnostics(), ...program.getBindDiagnostics()];
    if (inputDiagnostics.length) throw new Error('input errors');
    const node = semantic.getTokenAtPosition(program.getSourceFile(path.join(root, 'value.ts')), source.indexOf('Budget'));
    const type = project.checker.getDeclaredTypeOfSymbol(project.checker.getSymbolAtLocation(node));
    const properties = project.checker.getPropertiesOfType(type);
    if (properties.length !== 1 || properties[0].name !== key) throw new Error('property identity mismatch');
    const child = project.checker.getTypeOfSymbol(properties[0]);
    const row = { strict, length, sha256: crypto.createHash('sha256').update(source).digest('hex'), root: project.checker.typeToString(type), rootError: type.isErrorType(), child: project.checker.typeToString(child), childError: child.isErrorType() };
    rows.push(row);
    process.stdout.write(JSON.stringify(row) + '\n');
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}
fs.writeFileSync(path.join(directory, 'observations.json'), JSON.stringify({ version: semantic.version, binary: semantic.binary, rows }, null, 2) + '\n');

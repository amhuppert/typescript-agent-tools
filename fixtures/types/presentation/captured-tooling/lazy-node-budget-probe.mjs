import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadSemanticApi, lineStarts, position } from '../../../tools/semantic-oracle.mjs';

const root = path.dirname(fileURLToPath(import.meta.url));
const semantic = await loadSemanticApi();
assert.equal(semantic.version, '7.0.2');
const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const rows = [];
for (const count of [1995, 1996, 1997]) {
  const directory = path.join(root, String(count));
  fs.mkdirSync(directory, { recursive: true });
  const source = `export interface Budget { values: ${Array.from({ length: count }, (_, index) => index).join(' | ')}; late: Late; }\nexport interface Late { blocked: string extends number ? string : number; }\n`;
  fs.writeFileSync(path.join(directory, 'cases.ts'), source);
  const config = path.join(directory, 'tsconfig.json');
  fs.writeFileSync(config, JSON.stringify({ compilerOptions: { strict: true, noLib: true, noEmit: true, target: 'esnext', module: 'esnext' }, files: ['cases.ts'] }));
  const api = new semantic.API({ tsserverPath: semantic.binary, cwd: directory });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config);
    assert.ok(project);
    const inputDiagnostics = [project.program.getConfigFileParsingDiagnostics(), project.program.getProgramDiagnostics(), project.program.getSyntacticDiagnostics(), project.program.getBindDiagnostics()];
    assert.deepEqual(inputDiagnostics, [[], [], [], []]);
    const parsed = project.program.getSourceFile(path.join(directory, 'cases.ts'));
    assert.equal(parsed.text, source);
    const checker = project.checker;
    const declared = name => checker.getDeclaredTypeOfSymbol(checker.getSymbolAtLocation(parsed.statements.find(node => node.name?.text === name).name));
    const budget = declared('Budget');
    const budgetMembers = checker.getPropertiesOfType(budget);
    assert.deepEqual(budgetMembers.map(symbol => symbol.name), ['values', 'late']);
    const values = checker.getTypeOfSymbol(budgetMembers[0]);
    assert.equal(values.isUnionType(), true);
    assert.equal(values.getTypes().length, count);
    const late = checker.getTypeOfSymbol(budgetMembers[1]);
    const lateMembers = checker.getPropertiesOfType(late);
    assert.deepEqual(lateMembers.map(symbol => symbol.name), ['blocked']);
    const blocked = checker.getTypeOfSymbol(lateMembers[0]);
    assert.equal(checker.typeToString(blocked), 'number');
    assert.deepEqual([budget, values, late, blocked].map(type => type.isErrorType()), [false, false, false, false]);
    const annotation = parsed.statements.find(node => node.name?.text === 'Late').members[0].type;
    const starts = lineStarts(parsed.text);
    rows.push({ count, potentialNodes: count + 4, sourceSha256: digest(source), inputDiagnostics, root: checker.typeToString(budget), valuesConstituents: values.getTypes().length, late: checker.typeToString(late), blocked: checker.typeToString(blocked), blockedLocation: { file: 'cases.ts', start: position(starts, annotation.getStart(parsed)), end: position(starts, annotation.end) }, errorTypes: [budget, values, late, blocked].map(type => type.isErrorType()) });
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}
fs.writeFileSync(path.join(root, 'observations.json'), JSON.stringify({ version: semantic.version, binary: semantic.binary, binarySha256: digest(fs.readFileSync(semantic.binary)), semanticOracleSha256: digest(fs.readFileSync(new URL('../../../tools/semantic-oracle.mjs', import.meta.url))), probeSha256: digest(fs.readFileSync(fileURLToPath(import.meta.url))), rows }, null, 2) + '\n');
console.log(JSON.stringify(rows));

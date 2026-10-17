import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi, lineStarts, position } from './semantic-oracle.mjs';
import { typeOracle } from './type-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-parameter-selection/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'parameters.ts'), 'utf8');
const at = marker => ({ file: 'parameters.ts', ...position(lineStarts(source), source.indexOf(marker)) });

test('own parameter names select declared types while their actual type-reference uses select type questions', async () => {
  for (const [marker, question, summary, width] of [
    ['T extends MissingConstraint', 'declared', 'T', 1],
    ['T extends number', 'declared', 'T', 1],
    ['U> { first', 'declared', 'U', 1],
    ['U extends string', 'declared', 'U', 1],
    ['\\u0056 extends string', 'declared', '\\u0056', 6],
    ['T; }\ninterface Second', 'type', 'T', 1],
    ['\\u0056; }', 'type', '\\u0056', 6],
    ['V> { other', 'declared', '\\u0056', 1],
    ['W extends string', 'declared', 'W', 1],
    ['\\u0057> { second', 'declared', 'W', 6],
    ['\\u0057; }', 'type', 'W', 6],
  ]) {
    const result = await typeOracle(path.join(root, 'tsconfig.json'), { at: at(marker) });
    assert.equal(result.kind, question, marker);
    assert.equal(result.summary, summary, marker);
    assert.equal(result.errorType, false, marker);
    assert.deepEqual(result.prelude, [], marker);
    const { file, ...start } = at(marker);
    assert.equal(result.location.file, file, marker);
    assert.deepEqual(result.location.start, start, marker);
    assert.equal(result.location.end.column - result.location.start.column, width, marker);
  }
});

test('canonical merged parameters reuse one identity and their distinct source spellings retain raw anchors', async () => {
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  const api = new native.API({ tsserverPath: native.binary, cwd: root });
  let snapshot;
  try {
    const config = fs.realpathSync(path.join(root, 'tsconfig.json'));
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config);
    assert.ok(project);
    for (const diagnostics of [project.program.getConfigFileParsingDiagnostics(), project.program.getProgramDiagnostics(), project.program.getSyntacticDiagnostics(), project.program.getBindDiagnostics()])
      assert.deepEqual(diagnostics, []);
    const file = project.program.getSourceFile(path.join(root, 'parameters.ts'));
    assert.ok(file);
    const parameters = file.statements.map(node => node.typeParameters[0]);
    const symbols = parameters.map(parameter => project.checker.getSymbolAtLocation(parameter.name));
    assert.notEqual(symbols[0].id, symbols[1].id, 'unrelated same-named T parameters remain distinct');
    assert.equal(symbols[2].id, symbols[3].id, 'merged own U declarations bind one canonical symbol');
    const types = symbols.map(symbol => project.checker.getDeclaredTypeOfSymbol(symbol));
    assert.equal(types[2].id, types[3].id);
    assert.equal(symbols[4].id, symbols[5].id);
    assert.equal(types[4].id, types[5].id);
    assert.equal(symbols[6].id, symbols[7].id);
    assert.equal(types[6].id, types[7].id);
    assert.notEqual(types[0].id, types[1].id);
    assert.equal(types[0].isErrorType(), false, 'unused constraint/default recovery is not a parameter root dependency');
    assert.equal(project.checker.typeToString(types[4]), '\\u0056');
    assert.equal(symbols[4].name, 'V');
    assert.equal(parameters[4].name.text, 'V');
    assert.equal(file.text.slice(parameters[4].name.getStart(file), parameters[4].name.end), '\\u0056');
    assert.equal(project.checker.typeToString(types[5]), '\\u0056');
    assert.equal(project.checker.typeToString(types[6]), 'W');
    assert.equal(project.checker.typeToString(types[7]), 'W');
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
});

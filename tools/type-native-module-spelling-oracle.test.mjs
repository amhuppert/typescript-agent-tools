import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position, loadSemanticApi} from './semantic-oracle.mjs';

const fixture = fileURLToPath(new URL('../fixtures/types/native-module-spelling/', import.meta.url));
for (const host of ['config', 'workspace']) {
test(`native fresh module spelling: endings, mappings, packages and collision probes (${host} cwd)`, async () => {
  const cases = JSON.parse(fs.readFileSync(path.join(fixture, 'cases.json'), 'utf8'));
  const rows = [];
  for (const row of cases) {
    const root = path.resolve('.cc/temp/native-module-spelling', host, row.name);
    fs.rmSync(root, {recursive: true, force: true});
    for (const [file, text] of Object.entries({...row.files, 'globals.d.ts': fs.readFileSync(path.join(fixture, 'globals.d.ts'), 'utf8')})) {
      fs.mkdirSync(path.dirname(path.join(root, file)), {recursive: true});
      fs.writeFileSync(path.join(root, file), text);
    }
    const source = row.files[row.source];
    const at = marker => position(lineStarts(source), source.indexOf(marker) + marker.length);
    const config = path.join(root, row.config);
    const sourceFromConfig = path.relative(path.dirname(config), path.join(root, row.source));
    const currentDirectory = host === 'config' ? path.dirname(config) : process.cwd();
    const answer = await compilerHookOracle(config, {history: 'cold', currentDirectory, inspectSymbolDisplay: true,
      symbolDisplayEnclosing: true, at: {file: sourceFromConfig, ...at('/*#module*/')}});
    assert.equal(answer.compiler, '7.0.2');
    assert.deepEqual(answer.beforeSymbolDisplay.diagnosticCodes, [], row.name);
    assert.deepEqual(answer.diagnosticCodes, [], row.name);
    assert.deepEqual(answer.demandFailures, [], row.name);
    const native = await loadSemanticApi();
    const api = new native.API({tsserverPath: native.binary, cwd: currentDirectory});
    let snapshot;
    let presentSummary;
    try {
      snapshot = api.updateSnapshot({openProject: config});
      const project = snapshot.getProject(config);
      const input = project.program.getSourceFile(path.join(root, row.source));
      const marker = '/*#missing*/';
      const token = native.getTokenAtPosition(input, source.indexOf(marker) + marker.length);
      const symbol = project.checker.getSymbolAtLocation(token);
      assert.equal(project.checker.isUnknownSymbol(project.checker.getAliasedSymbol(symbol)), true, row.name);
      const [targetName, targetText] = Object.entries(row.files).find(([, text]) => text.startsWith('export declare const present:'));
      const target = project.program.getSourceFile(path.join(root, targetName));
      const present = native.getTokenAtPosition(target, targetText.indexOf('present'));
      presentSummary = project.checker.typeToString(project.checker.getTypeAtLocation(present));
    } finally { try { snapshot?.dispose(); } finally { api.close(); } }
    rows.push({name: row.name, display: Buffer.from(answer.symbolDisplayBytes, 'base64').toString('utf8'),
      defaultMode: answer.defaultResolutionMode, resolvedPath: answer.selectedModuleResolvedPath,
      imports: answer.importResolutionModes, presentSummary});
    fs.writeFileSync(`.cc/temp/native-module-spelling-${host}.json`, JSON.stringify(rows, null, 2) + '\n');
  }
  const expected = host === 'config' ? 'expected.json' : 'expected-workspace.json';
  assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(fixture, expected), 'utf8')));
});
}

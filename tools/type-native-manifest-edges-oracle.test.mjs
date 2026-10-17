import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position, loadSemanticApi} from './semantic-oracle.mjs';

const fixture = fileURLToPath(new URL('../fixtures/types/native-manifest-edges/', import.meta.url));
for (const host of ['config', 'workspace']) {
test(`native manifest edge values retain load decisions and fresh module spellings (${host} cwd)`, async () => {
  const cases = JSON.parse(fs.readFileSync(path.join(fixture, 'cases.json'), 'utf8'));
  const rows = [];
  for (const row of cases) {
    const root = path.resolve('.cc/temp/native-manifest-edges', host, row.name);
    fs.rmSync(root, {recursive: true, force: true});
    fs.mkdirSync(path.join(root, 'node_modules/pkg'), {recursive: true});
    const source = fs.readFileSync(path.join(fixture, 'main.cts'), 'utf8')
      .replace('from "pkg"', `from ${JSON.stringify(row.specifier ?? 'pkg')}`);
    fs.writeFileSync(path.join(root, 'main.cts'), source);
    fs.copyFileSync(path.join(fixture, 'globals.d.ts'), path.join(root, 'globals.d.ts'));
    fs.writeFileSync(path.join(root, 'package.json'), '{"type":"commonjs"}');
    fs.writeFileSync(path.join(root, 'node_modules/pkg/package.json'), row.manifest);
    for (const [file, value] of [['index', 'base'], ['first', 'first'], ['second', 'second']]) {
      fs.writeFileSync(path.join(root, `node_modules/pkg/${file}.d.ts`), `export declare const present: "${value}";\n`);
    }
    const config = path.join(root, 'tsconfig.json');
    fs.writeFileSync(config, JSON.stringify({compilerOptions: {strict: true, noLib: true, types: [],
      module: 'NodeNext', moduleResolution: 'NodeNext'}, files: ['globals.d.ts', 'main.cts']}));
    const currentDirectory = host === 'config' ? root : process.cwd();
    const native = await loadSemanticApi();
    const api = new native.API({tsserverPath: native.binary, cwd: currentDirectory});
    let snapshot;
    let selected;
    try {
      snapshot = api.updateSnapshot({openProject: config});
      const project = snapshot.getProject(config);
      const input = project.program.getSourceFile(path.join(root, 'main.cts'));
      const marker = '/*#selected*/';
      const token = native.getTokenAtPosition(input, source.indexOf(marker) + marker.length);
      const symbol = project.checker.getSymbolAtLocation(token);
      const target = project.checker.getAliasedSymbol(symbol);
      const type = project.checker.getTypeAtLocation(token);
      selected = {summary: project.checker.typeToString(type), errorType: type.isErrorType(),
        unknown: project.checker.isUnknownSymbol(target),
        loaded: project.program.getSourceFileNames().filter(file => file.includes('/node_modules/pkg/'))
          .map(file => path.relative(root, file).replaceAll(path.sep, '/')).sort()};
      const missingMarker = '/*#missing*/';
      const missingToken = native.getTokenAtPosition(input, source.indexOf(missingMarker) + missingMarker.length);
      const missing = project.checker.getSymbolAtLocation(missingToken);
      assert.equal(project.checker.isUnknownSymbol(project.checker.getAliasedSymbol(missing)), true, row.name);
    } finally { try { snapshot?.dispose(); } finally { api.close(); } }
    const marker = '/*#module*/';
    const display = await compilerHookOracle(config, {history: 'cold', currentDirectory, inspectSymbolDisplay: true,
      symbolDisplayEnclosing: true, at: {file: 'main.cts', ...position(lineStarts(source), source.indexOf(marker) + marker.length)}});
    assert.equal(display.compiler, '7.0.2');
    assert.deepEqual(display.beforeSymbolDisplay.diagnosticCodes, [], row.name);
    assert.deepEqual(display.diagnosticCodes, [], row.name);
    assert.deepEqual(display.demandFailures, [], row.name);
    assert.equal(display.importResolutionModes[0].resolvedPath, display.selectedModuleResolvedPath, row.name);
    rows.push({name: row.name, selected,
      display: Buffer.from(display.symbolDisplayBytes, 'base64').toString('utf8'),
      defaultMode: display.defaultResolutionMode, resolvedPath: display.selectedModuleResolvedPath,
      imports: display.importResolutionModes});
    fs.writeFileSync(`.cc/temp/native-manifest-edges-${host}.json`, JSON.stringify(rows, null, 2) + '\n');
  }
  const expected = host === 'config' ? 'expected.json' : 'expected-workspace.json';
  assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(fixture, expected), 'utf8')));
});
}

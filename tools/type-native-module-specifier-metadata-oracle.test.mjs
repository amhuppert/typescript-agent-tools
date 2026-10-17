import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position, loadSemanticApi} from './semantic-oracle.mjs';

const fixture = fileURLToPath(new URL('../fixtures/types/native-module-specifier-metadata/', import.meta.url));
const variants = [
  {name: 'dependencies', field: 'dependencies'},
  {name: 'peerDependencies', field: 'peerDependencies'},
  {name: 'optionalDependencies', field: 'optionalDependencies'},
  {name: 'devDependencies', field: 'devDependencies'},
  {name: 'invalidAfter', field: 'dependencies', dependencies: {pkg: '*', invalid: 42}},
  {name: 'invalidBefore', field: 'dependencies', dependencies: {invalid: 42, pkg: '*'}},
  {name: 'invalidPackageValue', field: 'dependencies', dependencies: {pkg: 42}},
  {name: 'nullPackageValue', field: 'dependencies', dependencies: {pkg: null}},
  {name: 'nonobjectRuntimeMap', field: 'dependencies', dependencies: ['pkg']},
  {name: 'noEmit', field: 'dependencies', noEmit: true},
  {name: 'declarationOnly', field: 'dependencies', declaration: true},
  {name: 'physicalSymlink', field: 'dependencies', symlink: true},
  {name: 'duplicateInvalidThenValid', packageJson: '{"dependencies":{"bad":0,"bad":"1","pkg":"1"}}'},
  {name: 'duplicateValidThenPackage', packageJson: '{"dependencies":{"bad":"1","bad":"1","pkg":"1"}}'},
  {name: 'duplicatePackage', packageJson: '{"dependencies":{"pkg":"1","pkg":0}}'},
  {name: 'repeatedMapRetains', packageJson: '{"dependencies":{"pkg":"1"},"dependencies":{"other":"1"}}'},
  {name: 'repeatedMapNullClears', packageJson: '{"dependencies":{"pkg":"1"},"dependencies":null}'},
  {name: 'repeatedMapInvalidRetains', packageJson: '{"dependencies":{"pkg":"1"},"dependencies":42}'},
  {name: 'repeatedMapAfterInvalid', packageJson: '{"dependencies":{"bad":0},"dependencies":{"pkg":"1"}}'},
];

test('native runtime dependency module paths retain original-empty and emit gates', async () => {
  const rows = [];
  for (const variant of variants) {
    const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'native-specifier-metadata-')));
    try {
      const file = variant.declaration ? 'main.d.cts' : 'main.cts';
      const source = fs.readFileSync(path.join(fixture, 'main.cts'), 'utf8');
      fs.writeFileSync(path.join(root, file), source);
      for (const name of ['globals.d.ts', 'index.d.ts']) fs.copyFileSync(path.join(fixture, name), path.join(root, name));
      const physical = variant.symlink ? 'physical/pkg' : 'node_modules/pkg';
      fs.mkdirSync(path.join(root, physical), {recursive: true});
      fs.copyFileSync(path.join(fixture, 'package-index.d.ts'), path.join(root, physical, 'index.d.ts'));
      fs.writeFileSync(path.join(root, physical, 'package.json'), JSON.stringify({name: 'pkg', types: './index.d.ts'}));
      if (variant.symlink) {
        fs.mkdirSync(path.join(root, 'node_modules'), {recursive: true});
        fs.symlinkSync('../physical/pkg', path.join(root, 'node_modules/pkg'), 'dir');
      }
      fs.writeFileSync(path.join(root, 'package.json'), variant.packageJson ?? JSON.stringify({type: 'commonjs',
        [variant.field]: variant.dependencies ?? {pkg: '*'}}));
      const config = path.join(root, 'tsconfig.json');
      fs.writeFileSync(config, JSON.stringify({compilerOptions: {strict: true, noLib: true, types: [],
        module: 'NodeNext', moduleResolution: 'NodeNext', noEmit: variant.noEmit === true},
        files: ['globals.d.ts', file]}));
      const marker = '/*#module*/';
      const native = await compilerHookOracle(config, {history: 'cold', inspectSymbolDisplay: true,
        symbolDisplayEnclosing: true, at: {file, ...position(lineStarts(source), source.indexOf(marker) + marker.length)}});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual(native.beforeSymbolDisplay.diagnosticCodes, []);
      assert.deepEqual(native.diagnosticCodes, []);
      assert.deepEqual(native.demandFailures, []);
      const implementation = await loadSemanticApi();
      const api = new implementation.API({tsserverPath: implementation.binary, cwd: root});
      let snapshot;
      try {
        snapshot = api.updateSnapshot({openProject: config});
        const project = snapshot.getProject(config);
        const input = project.program.getSourceFile(path.join(root, file));
        const marker = '/*#missing*/';
        const token = implementation.getTokenAtPosition(input, source.indexOf(marker) + marker.length);
        const alias = project.checker.getSymbolAtLocation(token);
        assert.equal(project.checker.isUnknownSymbol(project.checker.getAliasedSymbol(alias)), true);
      } finally { try { snapshot?.dispose(); } finally { api.close(); } }
      rows.push({name: variant.name, file,
        display: Buffer.from(native.symbolDisplayBytes, 'base64').toString('utf8'),
        defaultMode: native.defaultResolutionMode, resolvedPath: native.selectedModuleResolvedPath,
        imports: native.importResolutionModes});
    } finally { fs.rmSync(root, {recursive: true, force: true}); }
  }
  fs.mkdirSync('.cc/temp', {recursive: true});
  fs.writeFileSync('.cc/temp/native-module-specifier-metadata.json', JSON.stringify(rows, null, 2) + '\n');
  assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(fixture, 'expected.json'), 'utf8')));
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const fixture = fileURLToPath(new URL('../fixtures/types/native-module-specifier-snapshot/', import.meta.url));
test('native fresh module spelling reaches collision probes while written imports bypass them', async () => {
  const rows = [];
  for (const variant of [{file: 'fresh.cts'}, {file: 'reuse.cts'},
    {file: 'runtime.cts', runtime: true}, {file: 'runtime.cts', runtime: false}]) for (const collision of [false, true]) {
    const {file} = variant;
    const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'native-specifier-snapshot-')));
    try {
      const source = fs.readFileSync(path.join(fixture, file), 'utf8');
      fs.writeFileSync(path.join(root, file), source);
      fs.copyFileSync(path.join(fixture, 'globals.d.ts'), path.join(root, 'globals.d.ts'));
      fs.mkdirSync(path.join(root, 'node_modules/pkg/sub'), {recursive: true});
      fs.copyFileSync(path.join(fixture, 'target.d.ts'), path.join(root, 'node_modules/pkg/sub/index.d.ts'));
      if (file === 'runtime.cts') {
        fs.mkdirSync(path.join(root, 'sub'));
        fs.copyFileSync(path.join(fixture, 'target.d.ts'), path.join(root, 'sub/index.d.ts'));
      }
      fs.writeFileSync(path.join(root, 'node_modules/pkg/package.json'), JSON.stringify({name: 'pkg', types: './entry.d.ts'}));
      if (collision) fs.writeFileSync(path.join(root, 'node_modules/pkg/sub.ts'), 'export {};');
      fs.writeFileSync(path.join(root, 'package.json'), JSON.stringify({type: 'commonjs',
        ...(file === 'runtime.cts' ? {[variant.runtime ? 'dependencies' : 'devDependencies']: {pkg: '*'}} : {})}));
      const config = path.join(root, 'tsconfig.json');
      fs.writeFileSync(config, JSON.stringify({compilerOptions: {strict: true, noLib: true, types: [],
        module: 'NodeNext', moduleResolution: 'NodeNext'}, files: ['globals.d.ts', file]}));
      const marker = '/*#module*/';
      const answer = await compilerHookOracle(config, {history: 'cold', inspectSymbolDisplay: true,
        symbolDisplayEnclosing: true, at: {file, ...position(lineStarts(source), source.indexOf(marker) + marker.length)}});
      assert.deepEqual(answer.diagnosticCodes, []);
      assert.deepEqual(answer.demandFailures, []);
      assert.equal(answer.importResolutionModes[0].resolvedPath, answer.selectedModuleResolvedPath);
      rows.push({...variant, collision, display: Buffer.from(answer.symbolDisplayBytes, 'base64').toString('utf8'),
        defaultMode: answer.defaultResolutionMode, resolvedPath: answer.selectedModuleResolvedPath, imports: answer.importResolutionModes});
    } finally { fs.rmSync(root, {recursive: true, force: true}); }
  }
  fs.mkdirSync('.cc/temp', {recursive: true});
  fs.writeFileSync('.cc/temp/native-module-specifier-snapshot.json', JSON.stringify(rows, null, 2) + '\n');
  assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(fixture, 'expected.json'), 'utf8')));
});

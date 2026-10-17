import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { test } from 'node:test';
import { compilerHookOracle, typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const globals = fs.readFileSync(new URL('../fixtures/types/native-module-specifiers/globals.d.ts', import.meta.url), 'utf8');
function fixture(t, files, config = {}) {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'type-oracle-fs-')));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const [name, text] of Object.entries({ 'globals.d.ts': globals, ...files })) {
    fs.mkdirSync(path.dirname(path.join(root, name)), { recursive: true });
    fs.writeFileSync(path.join(root, name), text);
  }
  fs.writeFileSync(path.join(root, 'tsconfig.json'), JSON.stringify({
    compilerOptions: { noLib: true, strict: true, types: [], target: 'ESNext', module: 'Preserve', moduleResolution: 'Bundler' },
    files: ['globals.d.ts', ...Object.keys(files).filter(name => /cases\.[cm]?ts$/.test(name))], ...config,
  }));
  return root;
}
function request(root, file, display = false) {
  const source = fs.readFileSync(path.join(root, file), 'utf8').replace(/^\uFEFF/, '');
  const marker = '/*#query*/';
  assert.ok(source.includes(marker));
  return { history: 'cold', at: { file, ...position(lineStarts(source), source.indexOf(marker) + marker.length) },
    ...(display ? { inspectSymbolDisplay: true, symbolDisplayEnclosing: true } : {}) };
}
const display = answer => Buffer.from(answer.symbolDisplayBytes, 'base64').toString('utf8');

test('raw resolutions retain positive existence for unadmitted TSX and JS without inventing reads', async t => {
  const root = fixture(t, {
    'cases.ts': 'import "./other.tsx";\nimport "./loose.js";\nimport "./absent";\nimport "./empty";\nimport { present } from /*#query*/"./target.js";\n',
    'other.tsx': 'export const other = 1;\n', 'loose.js': 'export const other = 2;\n',
    'target.d.ts': 'export declare const present: 1;\n',
  });
  fs.mkdirSync(path.join(root, 'empty'));
  const answer = await compilerHookOracle(path.join(root, 'tsconfig.json'), request(root, 'cases.ts', true));
  assert.equal(display(answer), '"./target.js"');
  assert.deepEqual(answer.importResolutionModes.map(({ text, resolvedPath }) => [text, resolvedPath]), [
    ['./other.tsx', 'other.tsx'], ['./loose.js', 'loose.js'], ['./absent', null], ['./empty', null], ['./target.js', 'target.d.ts'],
  ]);
  assert.equal(answer.inputFiles.includes(path.join(root, 'other.tsx')), false, 'existence is not a content read');
  assert.equal(answer.inputFiles.includes(path.join(root, 'loose.js')), false, 'unadmitted JS remains unread');
  assert.deepEqual(answer.demandFailures, []);
});

test('directory enumeration, symlinks, empty contents and original BOM bytes replay the final type', async t => {
  const source = '\uFEFFimport { value } from "linked";\nexport const /*#query*/query = value;\n';
  const root = fixture(t, {
    'src/cases.ts': source, 'src/empty.ts': '',
    'package/package.json': '{"name":"linked","types":"index.d.ts"}',
    'package/index.d.ts': 'export declare const value: "linked-value";\n',
  }, { files: ['globals.d.ts'], include: ['src/**/*.ts'] });
  fs.mkdirSync(path.join(root, 'node_modules'));
  fs.symlinkSync(path.join(root, 'package'), path.join(root, 'node_modules', 'linked'), 'dir');
  const config = path.join(root, 'tsconfig.json'), at = request(root, 'src/cases.ts');
  const native = await typeOracle(config, at), answer = await compilerHookOracle(config, at);
  assert.equal(answer.summary, '"linked-value"');
  assert.equal(answer.summary, native.summary);
  for (const name of ['src/cases.ts', 'src/empty.ts', 'package/index.d.ts']) {
    const file = path.join(root, name), expected = createHash('sha256').update(fs.readFileSync(file)).digest('hex');
    assert.equal(answer.inputHashes.find(row => row.file === file)?.sha256, expected, name);
  }
  assert.deepEqual(answer.demandFailures, []);
});

for (const collision of [false, true]) {
  test(`hook-only index sibling probes restart a fresh replay (collision=${collision})`, async t => {
    const root = fixture(t, {
      'cases.cts': 'import type { present } from "./target/index" with { "resolution-mode": "import" };\nimport { present as other } from /*#query*/"./target/index";\n',
      'target/index.d.ts': 'export declare const present: 1;\n',
      ...(collision ? { 'target.ts': 'export const separate = 2;\n' } : {}),
    });
    const answer = await compilerHookOracle(path.join(root, 'tsconfig.json'), request(root, 'cases.cts', true));
    assert.equal(display(answer), collision ? '"./target/index"' : '"./target"');
    assert.ok(answer.fileSystemReplay.additionalOperations.some(probe => probe.operation === 'fileExists' && probe.path === path.join(root, 'target.ts')),
      JSON.stringify(answer.fileSystemReplay));
    assert.equal(answer.inputFiles.includes(path.join(root, 'target.ts')), false);
    assert.deepEqual(answer.demandFailures, []);
  });
}

test('bundled libraries retain their pinned virtual filesystem owner', async t => {
  const root = fixture(t, { 'cases.ts': 'export const /*#query*/query = ["text"].length;\n' }, {
    files: ['cases.ts'], compilerOptions: { strict: true, types: [], lib: ['es5'] },
  });
  const answer = await compilerHookOracle(path.join(root, 'tsconfig.json'), request(root, 'cases.ts'));
  assert.equal(answer.summary, 'number');
  assert.deepEqual(answer.demandFailures, []);
});

test('a later observation cannot overwrite the original captured source', async t => {
  const root = fixture(t, { 'cases.ts': 'export const /*#query*/query = 1;\n' });
  const at = request(root, 'cases.ts'), file = path.join(root, 'cases.ts');
  const readFile = fs.readFileSync;
  let reads = 0;
  fs.readFileSync = function (name, ...args) {
    if (name === file && ++reads === 2) fs.writeFileSync(file, 'export const /*#query*/query = 2;\n');
    return readFile.call(this, name, ...args);
  };
  try {
    await assert.rejects(compilerHookOracle(path.join(root, 'tsconfig.json'), at), /Oracle filesystem changed during capture/);
  } finally { fs.readFileSync = readFile; }
});

test('filesystem failures do not become absent module probes', async t => {
  const root = fixture(t, { 'cases.ts': 'import "./loop";\nexport const /*#query*/query = 1;\n' });
  fs.symlinkSync('loop.ts', path.join(root, 'loop.ts'));
  await assert.rejects(compilerHookOracle(path.join(root, 'tsconfig.json'), request(root, 'cases.ts')), /ELOOP|too many (levels of )?symbolic links/);
});

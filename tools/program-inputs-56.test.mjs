import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadTypeScript56, typeScript56Unavailable, TS56_VARIABLE } from './ts56-compiler.mjs';

// Real-compiler tests are skipped, with this reason, when the pinned
// TypeScript 5.6.3 package is not found.
const compilerUnavailable = typeScript56Unavailable();
// The 7.0.2 cross-check also needs both 7.0.2 compilers.
const semanticUnavailable = (!process.env.TYPE_SCRIPT_SEMANTIC_API || !process.env.TYPE_SCRIPT_NATIVE)
  && 'TYPE_SCRIPT_SEMANTIC_API and TYPE_SCRIPT_NATIVE are not both set';
const runner = fileURLToPath(new URL('./program-inputs.mjs', import.meta.url));

const OPTIONS = { noLib: true, types: [], strict: true, module: 'esnext', moduleResolution: 'bundler', esModuleInterop: true, incremental: true };
const FILES = {
  'tsconfig.json': JSON.stringify({ compilerOptions: OPTIONS }),
  'a.ts': 'export const value = 1;\n',
  'b.ts': 'import { value } from "./a";\nexport const doubled = value + value;\n',
};
// One package name and version installed twice: imported directly, and
// through package `a`, which has its own copy.
const DUPLICATE = {
  'tsconfig.json': JSON.stringify({ compilerOptions: OPTIONS, include: ['main.ts'] }),
  'main.ts': 'import { dup } from "dup";\nimport { dup as viaA } from "a";\nexport const both = dup + viaA;\n',
  'node_modules/dup/package.json': '{ "name": "dup", "version": "1.0.0", "types": "index.d.ts" }',
  'node_modules/dup/index.d.ts': 'export declare const dup: number;\n',
  'node_modules/a/package.json': '{ "name": "a", "version": "1.0.0", "types": "index.d.ts" }',
  'node_modules/a/index.d.ts': 'export { dup } from "dup";\n',
  'node_modules/a/node_modules/dup/package.json': '{ "name": "dup", "version": "1.0.0", "types": "index.d.ts" }',
  'node_modules/a/node_modules/dup/index.d.ts': 'export declare const dup: number;\n',
};

// The root copy imports `x`, whose own copy of `dup` redirects to the root
// copy while the root copy's subtree is still being processed: 5.6.3 lists
// that redirect before its target, which 7.0.2 lists after `x`.
const REDIRECT_FIRST = {
  'tsconfig.json': JSON.stringify({ compilerOptions: OPTIONS, include: ['main.ts'] }),
  'main.ts': 'import { which } from "dup";\nexport const v = which;\n',
  'node_modules/dup/package.json': '{ "name": "dup", "version": "1.0.0", "types": "index.d.ts" }',
  'node_modules/dup/index.d.ts': 'import "x";\nexport declare const which: "root";\n',
  'node_modules/x/package.json': '{ "name": "x", "version": "1.0.0", "types": "index.d.ts" }',
  'node_modules/x/index.d.ts': 'import "dup";\nexport {};\n',
  'node_modules/x/node_modules/dup/package.json': '{ "name": "dup", "version": "1.0.0", "types": "index.d.ts" }',
  'node_modules/x/node_modules/dup/index.d.ts': 'import "x";\nexport declare const which: "nested";\n',
};
const REDIRECT_FIRST_FILES = [['node_modules/x/index.d.ts', 'dependency'], ['node_modules/dup/index.d.ts', 'dependency'], ['main.ts', 'project']];

function project(t, files) {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-inputs-56-')));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const [file, text] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), text);
  }
  return { root, config: path.join(root, 'tsconfig.json') };
}

// Runs program-inputs on a hand-written listing in the format of the Rust
// test `list_configured_program`; the listing and report live outside the
// project, so the project holds only what the compilers write.
function comparer(t, config, options = ['--oracle=5.6'], env = {}) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-inputs-56-work-'));
  t.after(() => fs.rmSync(work, { recursive: true, force: true }));
  return (files, redirects) => {
    const listing = path.join(work, 'listing.json');
    const reportPath = path.join(work, 'report.json');
    fs.rmSync(reportPath, { force: true });
    fs.writeFileSync(listing, JSON.stringify({
      config, files: files.map(([file, origin]) => ({ file, origin })),
      ...(redirects && { redirects: redirects.map(([file, target]) => ({ file, target })) }),
    }, null, 2));
    const result = spawnSync(process.execPath, [runner, listing, config, ...options, `--report=${reportPath}`],
      { cwd: path.dirname(config), env: { ...process.env, ...env }, encoding: 'utf8', timeout: 60_000 });
    const report = fs.existsSync(reportPath) ? JSON.parse(fs.readFileSync(reportPath, 'utf8')) : undefined;
    return { status: result.status, stderr: result.stderr, report, kinds: report?.differences.map(difference => difference.kind) };
  };
}

const buildInfo = root => fs.readdirSync(root, { recursive: true }).map(String).filter(file => file.endsWith('.tsbuildinfo'));

test('real compiler: the 5.6 mode compares a listing with 5.6.3\'s program and writes no .tsbuildinfo', { skip: compilerUnavailable }, t => {
  const { root, config } = project(t, FILES);
  const compare = comparer(t, config);
  const matching = compare([['a.ts', 'project'], ['b.ts', 'project']]);
  assert.equal(matching.status, 0, matching.stderr);
  assert.equal(matching.report.compiler, 'Version 5.6.3');
  assert.deepEqual(matching.report.files, [{ file: 'a.ts', origin: 'project' }, { file: 'b.ts', origin: 'project' }]);
  assert.deepEqual(matching.report.redirects, []);
  assert.deepEqual(matching.kinds, []);
  assert.deepEqual(compare([['b.ts', 'project'], ['a.ts', 'project']]).kinds, ['order', 'order']);
  const provenance = compare([['a.ts', 'dependency'], ['b.ts', 'project']]);
  assert.deepEqual({ status: provenance.status, differences: provenance.report.differences },
    { status: 1, differences: [{ kind: 'provenance', file: 'a.ts', tool: 'dependency', compiler: 'project' }] });
  assert.deepEqual(compare([['a.ts', 'project']]).kinds, ['onlyCompiler', 'order']);

  // `incremental: true` is forced off, so nothing was written; a plain 5.6.3
  // check of the same project writes its build information.
  assert.deepEqual(buildInfo(root), []);
  spawnSync(process.execPath, [loadTypeScript56().tsc, '-p', config, '--noEmit'], { cwd: root, encoding: 'utf8', timeout: 60_000 });
  assert.deepEqual(buildInfo(root), ['tsconfig.tsbuildinfo']);
});

test('real compiler: the 5.6 mode names a library from the 5.6.3 package by its file name', { skip: compilerUnavailable }, t => {
  const { config } = project(t, {
    'tsconfig.json': JSON.stringify({ compilerOptions: { lib: ['es5'], types: [], strict: true } }),
    'a.ts': 'export const squares = [1, 2].map(n => n * n);\n',
  });
  const result = comparer(t, config)([
    ['lib.es5.d.ts', 'library'], ['lib.decorators.d.ts', 'library'], ['lib.decorators.legacy.d.ts', 'library'], ['a.ts', 'project'],
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.kinds, []);
});

test('real compiler: the 5.6 mode folds a package-id duplicate into its target', { skip: compilerUnavailable }, t => {
  const { root, config } = project(t, DUPLICATE);
  // 5.6.3's own listing names the second copy as its own input.
  const listed = spawnSync(process.execPath, [loadTypeScript56().tsc, '-p', config, '--listFilesOnly', '--incremental', 'false'],
    { cwd: root, encoding: 'utf8', timeout: 60_000 });
  assert.deepEqual(listed.stdout.split('\n').filter(Boolean).map(file => path.relative(root, file)), [
    'node_modules/dup/index.d.ts', 'node_modules/a/node_modules/dup/index.d.ts', 'node_modules/a/index.d.ts', 'main.ts',
  ]);

  const compare = comparer(t, config);
  const folded = compare([['node_modules/dup/index.d.ts', 'dependency'], ['node_modules/a/index.d.ts', 'dependency'], ['main.ts', 'project']]);
  assert.equal(folded.status, 0, folded.stderr);
  assert.deepEqual(folded.report.files, [
    { file: 'node_modules/dup/index.d.ts', origin: 'dependency' },
    { file: 'node_modules/a/index.d.ts', origin: 'dependency' },
    { file: 'main.ts', origin: 'project' },
  ]);
  assert.deepEqual(folded.report.redirects, [{ file: 'node_modules/a/node_modules/dup/index.d.ts', target: 'node_modules/dup/index.d.ts' }]);
  // A listing's redirects must be 5.6.3's.
  const files = [['node_modules/dup/index.d.ts', 'dependency'], ['node_modules/a/index.d.ts', 'dependency'], ['main.ts', 'project']];
  assert.deepEqual(compare(files, [['node_modules/a/node_modules/dup/index.d.ts', 'node_modules/dup/index.d.ts']]).kinds, []);
  assert.deepEqual(compare(files, []).report.differences, [
    { kind: 'redirect', file: 'node_modules/a/node_modules/dup/index.d.ts', tool: null, compiler: 'node_modules/dup/index.d.ts' },
  ]);
  assert.deepEqual(compare(files, [['node_modules/dup/index.d.ts', 'node_modules/a/node_modules/dup/index.d.ts']]).kinds, ['redirect', 'redirect']);
  // A listing that keeps the copy as an input of its own differs.
  const unfolded = compare([['node_modules/dup/index.d.ts', 'dependency'], ['node_modules/a/node_modules/dup/index.d.ts', 'dependency'],
    ['node_modules/a/index.d.ts', 'dependency'], ['main.ts', 'project']]);
  assert.deepEqual({ status: unfolded.status, kinds: unfolded.kinds }, { status: 1, kinds: ['onlyTool', 'order', 'order', 'order'] });
  assert.deepEqual(buildInfo(root), []);
});

test('real compiler: the 5.6 mode drops a redirect that 5.6.3 lists before its target', { skip: compilerUnavailable }, t => {
  const { root, config } = project(t, REDIRECT_FIRST);
  const listed = spawnSync(process.execPath, [loadTypeScript56().tsc, '-p', config, '--listFilesOnly', '--incremental', 'false'],
    { cwd: root, encoding: 'utf8', timeout: 60_000 });
  assert.deepEqual(listed.stdout.split('\n').filter(Boolean).map(file => path.relative(root, file)), [
    'node_modules/x/node_modules/dup/index.d.ts', 'node_modules/x/index.d.ts', 'node_modules/dup/index.d.ts', 'main.ts',
  ]);
  const result = comparer(t, config)(REDIRECT_FIRST_FILES);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.kinds, []);
  assert.deepEqual(result.report.redirects, [{ file: 'node_modules/x/node_modules/dup/index.d.ts', target: 'node_modules/dup/index.d.ts' }]);
});

test('real compilers: 5.6.3\'s folded program is the one TypeScript 7.0.2 lists', { skip: compilerUnavailable || semanticUnavailable }, t => {
  const cases = [
    [DUPLICATE, [['node_modules/dup/index.d.ts', 'dependency'], ['node_modules/a/index.d.ts', 'dependency'], ['main.ts', 'project']]],
    [REDIRECT_FIRST, REDIRECT_FIRST_FILES],
  ];
  for (const [files, listing] of cases) {
    const { config } = project(t, files);
    for (const options of [['--oracle=5.6'], []]) {
      const result = comparer(t, config, options)(listing);
      assert.deepEqual({ status: result.status, kinds: result.kinds, stderr: result.stderr }, { status: 0, kinds: [], stderr: '' }, options.join(' '));
    }
  }
});

test('the 5.6 mode cannot run without the 5.6.3 package, and other oracle values are usage errors (exit 2)', t => {
  const { root, config } = project(t, FILES);
  const listing = [['a.ts', 'project'], ['b.ts', 'project']];
  const missing = path.join(root, 'no-compiler');
  assert.deepEqual(comparer(t, config, ['--oracle=5.6'], { [TS56_VARIABLE]: missing })(listing),
    { status: 2, stderr: `program-inputs: ${TS56_VARIABLE} names ${missing}, which holds no package.json\n`, report: undefined, kinds: undefined });
  const other = path.join(root, 'other');
  fs.mkdirSync(other);
  fs.writeFileSync(path.join(other, 'package.json'), '{ "name": "typescript", "version": "5.6.2" }');
  assert.deepEqual(comparer(t, config, ['--oracle=5.6'], { [TS56_VARIABLE]: other })(listing),
    { status: 2, stderr: `program-inputs: expected TypeScript 5.6.3 at ${other}, found version 5.6.2\n`, report: undefined, kinds: undefined });
  for (const option of ['--oracle=5.7', '--oracle=']) {
    assert.deepEqual(comparer(t, config, [option])(listing), {
      status: 2, report: undefined, kinds: undefined,
      stderr: 'usage: node tools/program-inputs.mjs LISTING.json TSCONFIG [--oracle=5.6] [--report=PATH]\n',
    });
  }
});

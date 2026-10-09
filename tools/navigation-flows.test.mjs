import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { runFlows } from './navigation-flows.mjs';
import { census, runSelection } from './name-selection.mjs';

// The real-project proof tools on a small project. They drive a built
// `tsparse`; without one these tests are skipped with this reason.
const tsparse = process.env.TSPARSE ?? fileURLToPath(new URL('../target/debug/tsparse', import.meta.url));
const unbuilt = !fs.existsSync(tsparse) && `no tsparse at ${tsparse}; build it or set TSPARSE`;
const unixOnly = process.platform === 'win32' && 'the project service is Unix-only';

// Real-compiler tests are skipped, with this reason, unless both compilers are configured.
const compilerUnavailable = (!process.env.TYPE_SCRIPT_SEMANTIC_API || !process.env.TYPE_SCRIPT_NATIVE)
  && 'TYPE_SCRIPT_SEMANTIC_API and TYPE_SCRIPT_NATIVE are not both set';

const FILES = {
  'tsconfig.json': '{ "compilerOptions": { "noLib": true, "types": [], "module": "esnext", "moduleResolution": "bundler" }, "include": ["src"] }',
  'src/a.ts': 'export function run(): void;\nexport function run(value?: number) {}\nexport function unique() {}\nexport const helper = () => {\n  function inner() {}\n  return inner;\n};\n',
  'src/b.ts': 'import { run, unique } from "./a";\nexport namespace Tools {\n  export function run() {}\n}\nexport { unique as renamed };\nrun();\nunique();\n',
  'src/c.ts': 'export function shared() {}\n',
  'src/d.ts': 'import { helper } from "./a";\nexport function shared() {}\nhelper();\n',
};

function project(t, files = FILES) {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-flows-')));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const [file, text] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), text);
  }
  return { root, config: path.join(root, 'tsconfig.json') };
}

test('a name census counts exported declarations a name selects alone', () => {
  const entry = (key, kind, signature, children = []) => ({ key, kind, signature, anchor: { line: 1, column: 1 }, children });
  const result = census([
    { file: 'a.ts', entries: [entry('f', 'function', 'export function f()'), entry('g', 'function', 'export function g()'),
      entry('A.B.N', 'namespace', 'export namespace A.B.N', [entry('local', 'variable', 'const local = 1')]),
      entry('h', 'function', 'function h()')] },
    { file: 'b.ts', entries: [entry('g', 'import', undefined), entry('N', 'export', undefined), entry('local', 'variable', 'export const local = 2')] },
  ]);
  // `local` is declared twice; `g` and `N` share their names with an import
  // and an export; `h` is not exported.
  assert.deepEqual(result.measured.map(declaration => declaration.name), ['f', 'g', 'N']);
  assert.deepEqual(result.ambiguous.map(declaration => declaration.name), ['g', 'N']);
  assert.equal(result.found, 1);
});

test('built tsparse: navigation flows and name selection hold on a small project', { skip: unbuilt || unixOnly }, t => {
  const { root, config } = project(t);
  const report = runFlows({
    tsparse, config, files: ['src/a.ts', 'src/b.ts', 'src/d.ts'], directSample: 3,
    requests: ['--references --name run', '--show --name run', '--outline src/b.ts'],
  });
  assert.deepEqual(report.failures, []);
  assert.equal(report.summary.files, 3);
  assert.ok(report.summary.ambiguous > 0 && report.summary.found > 0, JSON.stringify(report.summary));
  assert.equal(report.summary.directCompared, 6);
  assert.deepEqual(report.requests.map(request => request.status), [0, 1, 0]);

  // `unique` and `helper` are imported elsewhere without renaming, and
  // `Tools` is declared once; `run` and `shared` are declared twice.
  const selection = runSelection({ tsparse, config, files: ['src/a.ts', 'src/b.ts', 'src/c.ts', 'src/d.ts'], sample: 3 });
  assert.deepEqual(selection.failures, []);
  assert.deepEqual(selection.summary, { measured: 3, found: 3, ambiguous: 0, sampled: 3, sampledFound: 3, failures: 0 });
});

test('built tsparse: a direct answer that differs from the service\'s fails the flows', { skip: unbuilt || unixOnly }, t => {
  const { root, config } = project(t);
  // A stand-in that alters every direct answer.
  const wrapper = path.join(root, 'tsparse-wrapper');
  fs.writeFileSync(wrapper, `#!/bin/sh\ncase " $* " in *" --direct "*) "${tsparse}" "$@"; status=$?; echo extra; exit $status;; esac\nexec "${tsparse}" "$@"\n`, { mode: 0o755 });
  const report = runFlows({ tsparse: wrapper, config, files: ['src/a.ts'], directSample: 1, requests: ['--references --name run'] });
  assert.deepEqual(report.failures.map(failure => failure.what), [
    `--direct differs: --show --project ${config} --at 1:17 src/a.ts --json`,
    `--direct differs: --references --name run --project ${config}`,
  ]);
  const result = spawnSync(process.execPath, [fileURLToPath(new URL('./navigation-flows.mjs', import.meta.url)), config,
    `--report=${path.join(root, 'report.json')}`, '--request=--references --name run'],
  { cwd: root, env: { ...process.env, TSPARSE: wrapper }, encoding: 'utf8' });
  assert.equal(result.status, 1, result.stdout + result.stderr);
  assert.equal(JSON.parse(fs.readFileSync(path.join(root, 'report.json'), 'utf8')).summary.failures, 1);
});

test('built tsparse: text candidates round-trip through paths with spaces and colons', { skip: unbuilt || unixOnly }, t => {
  const { config } = project(t, {
    'tsconfig.json': '{ "compilerOptions": { "noLib": true, "types": [] }, "files": ["dir one/a b.ts", "dir one/c:d.ts"] }',
    'dir one/a b.ts': 'export function run() {}\n',
    'dir one/c:d.ts': 'export function run() {}\n',
  });
  const report = runFlows({ tsparse, config, files: ['dir one/a b.ts'], directSample: 0, requests: [] });
  assert.deepEqual(report.failures, []);
  assert.equal(report.summary.ambiguous, 1);
  // Two JSON candidates and two text candidates select their declarations.
  assert.equal(report.summary.roundTrips, 4);
});

test('built tsparse: a text ambiguity without its candidates fails the flows', { skip: unbuilt || unixOnly }, t => {
  const { root, config } = project(t);
  // A stand-in that drops the diagnostic of every text name show.
  const wrapper = path.join(root, 'tsparse-wrapper');
  fs.writeFileSync(wrapper, `#!/bin/sh\ncase " $* " in *" --json "*|*" --at "*) exec "${tsparse}" "$@";; *" --show "*" --name "*) "${tsparse}" "$@" >/dev/null 2>&1; exit $?;; esac\nexec "${tsparse}" "$@"\n`, { mode: 0o755 });
  const report = runFlows({ tsparse: wrapper, config, files: ['src/a.ts'], directSample: 0, requests: [] });
  assert.ok(report.failures.some(failure => failure.what.startsWith('text candidates for')), JSON.stringify(report.failures));
});

test('real compiler: program-inputs compares a listing with the compiler\'s program', { skip: compilerUnavailable }, t => {
  const { root, config } = project(t, {
    'tsconfig.json': '{ "compilerOptions": { "noLib": true, "types": [] }, "include": ["src"] }',
    'src/a.ts': 'export const a = 1;\n',
    'src/b.ts': 'export const b = 2;\n',
  });
  const runner = fileURLToPath(new URL('./program-inputs.mjs', import.meta.url));
  const compare = files => {
    const listing = path.join(root, 'listing.json');
    fs.writeFileSync(listing, JSON.stringify({ config, files: files.map(([file, origin]) => ({ file, origin })) }));
    const result = spawnSync(process.execPath, [runner, listing, config, `--report=${path.join(root, 'report.json')}`],
      { cwd: root, encoding: 'utf8', timeout: 60_000 });
    const report = JSON.parse(fs.readFileSync(path.join(root, 'report.json'), 'utf8'));
    return { status: result.status, kinds: report.differences.map(difference => difference.kind) };
  };
  assert.deepEqual(compare([['src/a.ts', 'project'], ['src/b.ts', 'project']]), { status: 0, kinds: [] });
  assert.deepEqual(compare([['src/b.ts', 'project'], ['src/a.ts', 'project']]), { status: 1, kinds: ['order', 'order'] });
  assert.deepEqual(compare([['src/a.ts', 'dependency'], ['src/b.ts', 'project']]), { status: 1, kinds: ['provenance'] });
});

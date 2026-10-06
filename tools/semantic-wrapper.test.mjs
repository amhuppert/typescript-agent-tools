import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const wrapper = fileURLToPath(new URL('../scripts/validate/semantic-full.sh', import.meta.url));
const platform = `${process.platform}-${process.arch}`;

// A temporary checkout with a linked worktree, as Command Center runs the
// wrapper, plus a stand-in Cargo and conformance script that record calls.
function fixture(t) {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-semantic-wrapper-')));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const git = (...args) => {
    const result = spawnSync('git', ['-c', 'user.name=t', '-c', 'user.email=t@example.com', ...args], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
  };
  const canonical = path.join(directory, 'canonical');
  const worktree = path.join(directory, 'worktree');
  fs.mkdirSync(canonical);
  git('-C', canonical, 'init', '-q');
  git('-C', canonical, 'commit', '-q', '--allow-empty', '-m', 'initial');
  git('-C', canonical, 'worktree', 'add', '-q', worktree);
  const log = path.join(directory, 'calls.jsonl');
  const record = `require('node:fs').appendFileSync(${JSON.stringify(log)}, JSON.stringify({ tool, args: process.argv.slice(2),
    api: process.env.TYPE_SCRIPT_SEMANTIC_API, native: process.env.TYPE_SCRIPT_NATIVE, batch: process.env.REFERENCE_BATCH,
    nodeOptions: process.env.NODE_OPTIONS }) + '\\n');`;
  const bin = path.join(directory, 'bin');
  fs.mkdirSync(bin);
  fs.writeFileSync(path.join(bin, 'cargo'), `#!${process.execPath}\nconst tool = 'cargo';\n${record}\n`, { mode: 0o755 });
  fs.mkdirSync(path.join(worktree, 'tools'));
  fs.writeFileSync(path.join(worktree, 'tools/semantic-conformance.mjs'), `const tool = 'conformance';
const require = (await import('node:module')).createRequire(import.meta.url);
${record}
if (process.env.TSPARSE_CONFORMANCE_FAIL) { console.log('FAIL fixture: differences'); console.error('details'); process.exit(1); }
`);
  const install = (name, file) => {
    const target = path.join(canonical, '.cache/compiler-oracles', name);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, '', { mode: 0o755 });
    return target;
  };
  const run = (environment = {}) => {
    fs.writeFileSync(log, '');
    const env = { ...process.env, PATH: `${bin}${path.delimiter}${process.env.PATH}`, NODE_OPTIONS: '--max-old-space-size=9999', ...environment };
    delete env.TYPE_SCRIPT_SEMANTIC_API;
    delete env.TYPE_SCRIPT_NATIVE;
    delete env.CARGO_TARGET_DIR;
    Object.assign(env, environment);
    const result = spawnSync(wrapper, [], { cwd: environment.CWD ?? worktree, env, encoding: 'utf8', timeout: 30_000 });
    assert.equal(result.error, undefined);
    const calls = fs.readFileSync(log, 'utf8').split('\n').filter(Boolean).map(line => JSON.parse(line));
    return { ...result, calls };
  };
  return { directory, canonical, worktree, install, run };
}

test('compilers in the canonical checkout cache are found from a linked worktree', t => {
  const { canonical, install, run } = fixture(t);
  const api = path.dirname(install('typescript-7.0.2/package/package.json'));
  const native = install(`typescript-${platform}-7.0.2/package/lib/tsc`);
  install('typescript-other-arch-7.0.2/package/lib/tsc');
  const result = run();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, '');
  assert.equal(result.stderr, '');
  assert.equal(api, path.join(canonical, '.cache/compiler-oracles/typescript-7.0.2/package'));
  assert.deepEqual(result.calls.map(call => [call.tool, ...call.args]), [
    ['cargo', 'build', '--offline', '--jobs', '2', '--example', 'reference_batch'],
    ['conformance', 'markers'],
    ['conformance', 'compare'],
  ]);
  for (const call of result.calls.slice(1)) {
    assert.deepEqual([call.api, call.native, call.batch, call.nodeOptions],
      [api, native, 'target/debug/examples/reference_batch', '--max-old-space-size=2048']);
  }
});

test('a single native package for another platform name is used; several are ambiguous', t => {
  const { install, run } = fixture(t);
  install('typescript-7.0.2/package/package.json');
  const only = install('typescript-elsewhere-7.0.2/package/lib/tsc');
  const result = run();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.calls[1].native, only);
  install('typescript-another-7.0.2/package/lib/tsc');
  const ambiguous = run();
  assert.equal(ambiguous.status, 2);
  assert.match(ambiguous.stderr, /several others/);
  assert.deepEqual(ambiguous.calls, []);
});

test('explicit compiler variables take precedence over the cache', t => {
  const { install, run } = fixture(t);
  install('typescript-7.0.2/package/package.json');
  install(`typescript-${platform}-7.0.2/package/lib/tsc`);
  const result = run({ TYPE_SCRIPT_SEMANTIC_API: '/explicit/api', TYPE_SCRIPT_NATIVE: '/explicit/tsc', CARGO_TARGET_DIR: '/build' });
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual([result.calls[1].api, result.calls[1].native, result.calls[1].batch],
    ['/explicit/api', '/explicit/tsc', '/build/debug/examples/reference_batch']);
});

test('missing compilers fail with exit 2 and an explicit message instead of passing', t => {
  const { directory, install, run } = fixture(t);
  for (const [environment, reason] of [
    [{}, /no semantic API package at .*typescript-7\.0\.2\/package/],
    [{ CWD: directory }, /no Git checkout was found/],
  ]) {
    const result = run(environment);
    assert.equal(result.status, 2, result.stderr);
    assert.equal(result.stdout, '');
    assert.match(result.stderr, reason);
    assert.match(result.stderr, /comparison did not run/);
    assert.deepEqual(result.calls, []);
  }
  install('typescript-7.0.2/package/package.json');
  const result = run();
  assert.equal(result.status, 2);
  assert.match(result.stderr, /no native compiler at .*typescript-.*-7\.0\.2\/package\/lib\/tsc/);
});

test('oracle differences fail with the complete output', t => {
  const { install, run } = fixture(t);
  install('typescript-7.0.2/package/package.json');
  install(`typescript-${platform}-7.0.2/package/lib/tsc`);
  const result = run({ TSPARSE_CONFORMANCE_FAIL: '1' });
  assert.equal(result.status, 1);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, /FAIL fixture: differences\ndetails/);
  assert.deepEqual(result.calls.map(call => call.args[0]), ['build', 'markers']);
});

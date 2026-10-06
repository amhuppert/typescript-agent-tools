import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const wrapper = fileURLToPath(new URL('../scripts/validate/test-changed.sh', import.meta.url));

function fixture(t) {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-rust-wrapper-')));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const bin = path.join(directory, 'bin');
  const log = path.join(directory, 'cargo-calls.json');
  fs.mkdirSync(bin);
  for (const file of ['tests/cli.rs', 'tests/literals.rs', 'tests/nested/main.rs', 'src/lib.rs']) {
    const target = path.join(directory, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, '');
  }
  fs.writeFileSync(path.join(bin, 'cargo'), `#!/usr/bin/env node
const fs = require('node:fs');
const log = process.env.TSPARSE_WRAPPER_LOG;
const calls = JSON.parse(fs.readFileSync(log, 'utf8'));
calls.push({
  args: process.argv.slice(2), cwd: process.cwd(),
  threads: process.env.RUST_TEST_THREADS, nodeOptions: process.env.NODE_OPTIONS,
  forceColor: process.env.FORCE_COLOR, noColor: process.env.NO_COLOR,
});
fs.writeFileSync(log, JSON.stringify(calls));
console.log('cargo stdout diagnostic');
console.error('cargo stderr diagnostic');
if (process.env.TSPARSE_WRAPPER_SKIP) console.log(process.env.TSPARSE_WRAPPER_SKIP);
process.exit(Number(process.env.TSPARSE_WRAPPER_EXIT ?? 0));
`, { mode: 0o755 });

  return (args, exit = 0, skip = '') => {
    fs.writeFileSync(log, '[]');
    const result = spawnSync(wrapper, args, {
      cwd: directory,
      env: {
        ...process.env,
        PATH: `${bin}${path.delimiter}${process.env.PATH}`,
        TSPARSE_WRAPPER_LOG: log,
        TSPARSE_WRAPPER_EXIT: String(exit),
        TSPARSE_WRAPPER_SKIP: skip,
        RUST_TEST_THREADS: '99',
        NODE_OPTIONS: '--max-old-space-size=8192',
        FORCE_COLOR: '1',
        NO_COLOR: '',
      },
      encoding: 'utf8', timeout: 10_000,
    });
    assert.equal(result.error, undefined, result.stderr);
    assert.equal(result.signal, null, result.stderr);
    return { ...result, calls: JSON.parse(fs.readFileSync(log, 'utf8')), directory };
  };
}

test('an explicit integration-test file selects only its Cargo target and pins resources', t => {
  const result = fixture(t)(['tests/cli.rs']);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.calls, [{
    args: ['test', '--offline', '--jobs', '2', '--test', 'cli', '--', '--test-threads=2', '--show-output'],
    cwd: result.directory,
    threads: '2', nodeOptions: '--max-old-space-size=2048', forceColor: '0', noColor: '1',
  }]);
  assert.equal(result.stdout, 'rust tests: passed, 0 skipped\n');
  assert.equal(result.stderr, '');
});

test('multiple explicit files select exactly their integration-test targets', t => {
  const result = fixture(t)(['tests/cli.rs', 'tests/literals.rs']);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.calls.length, 1);
  assert.deepEqual(result.calls[0].args,
    ['test', '--offline', '--jobs', '2', '--test', 'cli', '--test', 'literals', '--', '--test-threads=2', '--show-output']);
});

test('without explicit paths the changed wrapper uses the safe full-suite fallback', t => {
  const result = fixture(t)([]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.calls.length, 1);
  assert.deepEqual(result.calls[0].args,
    ['test', '--offline', '--jobs', '2', '--', '--test-threads=2', '--show-output']);
  assert.equal(result.stdout, 'rust tests: passed, 0 skipped\n');
  assert.equal(result.stderr, '');
});

test('a test that cannot run on this platform is reported as a skip', t => {
  const skip = 'SKIP case_policy: case-sensitive file system';
  const result = fixture(t)(['tests/cli.rs'], 0, skip);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, `rust tests: passed, 1 skipped on this platform:\n${skip}\n`);
});

test('invalid or unsupported paths fail before Cargo runs instead of widening scope', t => {
  const run = fixture(t);
  for (const args of [
    ['tests'], ['src/lib.rs'], ['tests/missing.rs'], ['tests/nested/main.rs'],
    ['tests/*.rs'], ['--all'], ['../tests/cli.rs'], ['/tests/cli.rs'],
    ['tests/cli.rs', 'tests/missing.rs'],
  ]) {
    const result = run(args);
    assert.equal(result.status, 2, `${args}: ${result.stderr}`);
    assert.deepEqual(result.calls, [], String(args));
    assert.equal(result.stdout, '');
    assert.match(result.stderr, /expected an existing integration-test file/);
  }
});

test('a failed Cargo run preserves its exit code and all diagnostics', t => {
  const result = fixture(t)(['tests/cli.rs'], 3);
  assert.equal(result.status, 3);
  assert.equal(result.calls.length, 1);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, /cargo stdout diagnostic/);
  assert.match(result.stderr, /cargo stderr diagnostic/);
});

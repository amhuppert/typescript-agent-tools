import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const changedWrapper = fileURLToPath(new URL('../scripts/validate/test-changed.sh', import.meta.url));
const fullWrapper = fileURLToPath(new URL('../scripts/validate/test-full.sh', import.meta.url));

// The resource profile both wrappers pin.
const JOBS = '4';
const THREADS = '2';
const BUILD = ['test', '--offline', '--jobs', JOBS, '--no-run', '--message-format=json-render-diagnostics'];
const DOC = ['test', '--offline', '--jobs', JOBS, '--doc', '--', `--test-threads=${THREADS}`, '--show-output'];
const RUN = [`--test-threads=${THREADS}`, '--show-output'];
const PROFILE = { threads: THREADS, nodeOptions: '--max-old-space-size=2048', forceColor: '0', noColor: '1' };

// A stand-in cargo. `--no-run` writes one stand-in test executable per
// selected target and reports it as Cargo's JSON artifact messages do;
// `--doc` reports doc tests. Every call and every executable run is logged.
const FAKE_CARGO = `#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const env = process.env;
const args = process.argv.slice(2);
const log = entry => fs.appendFileSync(env.TSPARSE_WRAPPER_LOG, JSON.stringify(entry) + '\\n');
log({ tool: 'cargo', args, cwd: process.cwd(), threads: env.RUST_TEST_THREADS,
  nodeOptions: env.NODE_OPTIONS, forceColor: env.FORCE_COLOR, noColor: env.NO_COLOR });
if (args.includes('--doc')) {
  console.log('test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s');
  process.exit(0);
}
console.log('cargo stdout diagnostic');
console.error('cargo stderr diagnostic');
if (env.TSPARSE_WRAPPER_BUILD_EXIT) process.exit(Number(env.TSPARSE_WRAPPER_BUILD_EXIT));
const targets = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--lib') targets.push(['lib', 'typescript_parser', 'src/lib.rs']);
  if (args[i] === '--bin') targets.push(['bin', args[++i], 'src/main.rs']);
  if (args[i] === '--test') { targets.push(['test', args[i + 1], 'tests/' + args[i + 1] + '.rs']); i++; }
}
if (targets.length === 0) {
  targets.push(['lib', 'typescript_parser', 'src/lib.rs'], ['bin', 'tsparse', 'src/main.rs']);
  for (const file of fs.readdirSync('tests').filter(file => file.endsWith('.rs')).sort()) {
    targets.push(['test', file.slice(0, -3), 'tests/' + file]);
  }
}
fs.mkdirSync(env.TSPARSE_WRAPPER_TARGETS, { recursive: true });
console.log(JSON.stringify({ reason: 'compiler-artifact', target: { name: 'tsparse', kind: ['bin'], src_path: path.join(process.cwd(), 'src/main.rs') },
  profile: { test: false }, executable: '/not/a/test/executable' }));
for (const [kind, name, label] of targets) {
  const executable = path.join(env.TSPARSE_WRAPPER_TARGETS, kind + '-' + name);
  fs.writeFileSync(executable, '#!/usr/bin/env node\\nrequire(process.env.TSPARSE_WRAPPER_EXECUTABLE)(' + JSON.stringify(label) + ');\\n', { mode: 0o755 });
  console.log(JSON.stringify({ reason: 'compiler-artifact', target: { name, kind: [kind], src_path: path.join(process.cwd(), label) },
    profile: { test: true }, executable }));
}
console.log(JSON.stringify({ reason: 'build-finished', success: true }));
`;

// What every stand-in test executable does, by label.
const FAKE_EXECUTABLE = `const fs = require('node:fs');
module.exports = label => {
  const env = process.env;
  const started = Date.now();
  const finish = () => {
    if (env.TSPARSE_WRAPPER_TOUCH) fs.appendFileSync(env.TSPARSE_WRAPPER_TOUCH, '// touched\\n');
    fs.appendFileSync(env.TSPARSE_WRAPPER_LOG, JSON.stringify({ tool: label, args: process.argv.slice(2),
      cwd: process.cwd(), started, ended: Date.now() }) + '\\n');
    console.log(label + ' output');
    if (env.TSPARSE_WRAPPER_SKIP) console.log(env.TSPARSE_WRAPPER_SKIP + ' in ' + label);
    if (env.TSPARSE_WRAPPER_FAIL === label) {
      console.log('test failing_case ... FAILED');
      console.error(label + ' failing diagnostic');
      console.log('test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s');
      process.exit(101);
    }
    console.log('test result: ok. ' + Number(env.TSPARSE_WRAPPER_PASSED ?? 1) + ' passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s');
  };
  const sleep = Number(env.TSPARSE_WRAPPER_SLEEP_MS ?? 0);
  if (sleep > 0) setTimeout(finish, sleep); else finish();
};
`;

/// A temporary directory holding the stand-in tools beside a project
/// directory `repository`, so nothing the tools write is a project change.
function workspace(t, files) {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-rust-wrapper-')));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const repository = path.join(root, 'repository');
  const bin = path.join(root, 'bin');
  const log = path.join(root, 'calls.ndjson');
  fs.mkdirSync(bin, { recursive: true });
  for (const [file, text] of Object.entries(files)) {
    const target = path.join(repository, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, text);
  }
  fs.writeFileSync(path.join(bin, 'cargo'), FAKE_CARGO, { mode: 0o755 });
  fs.writeFileSync(path.join(root, 'executable.js'), FAKE_EXECUTABLE);

  const run = (args, { wrapper = changedWrapper, ...options } = {}) => {
    fs.writeFileSync(log, '');
    const result = spawnSync(wrapper, args, {
      cwd: repository,
      env: {
        ...process.env,
        PATH: `${bin}${path.delimiter}${process.env.PATH}`,
        TSPARSE_WRAPPER_LOG: log,
        TSPARSE_WRAPPER_TARGETS: path.join(root, 'targets'),
        TSPARSE_WRAPPER_EXECUTABLE: path.join(root, 'executable.js'),
        TSPARSE_WRAPPER_BUILD_EXIT: options.buildExit ?? '',
        TSPARSE_WRAPPER_FAIL: options.fail ?? '',
        TSPARSE_WRAPPER_PASSED: String(options.passed ?? 1),
        TSPARSE_WRAPPER_SKIP: options.skip ?? '',
        TSPARSE_WRAPPER_SLEEP_MS: String(options.sleep ?? 0),
        TSPARSE_WRAPPER_TOUCH: options.touch ?? '',
        TARGET_BRANCH: 'main',
        CARGO_TARGET_DIR: options.targetDir ?? '',
        RUST_TEST_THREADS: '99',
        NODE_OPTIONS: '--max-old-space-size=8192',
        FORCE_COLOR: '1',
        NO_COLOR: '',
      },
      // This fixture checks forwarding and exit behavior, not process-start
      // latency. Cold child startup can exceed ten seconds on a loaded host.
      encoding: 'utf8', timeout: 60_000,
    });
    assert.equal(result.error, undefined, result.stderr);
    assert.equal(result.signal, null, result.stderr);
    const entries = fs.readFileSync(log, 'utf8').split('\n').filter(Boolean).map(line => JSON.parse(line));
    const calls = entries.filter(entry => entry.tool === 'cargo')
      .map(({ tool, ...call }) => call);
    const runs = entries.filter(entry => entry.tool !== 'cargo');
    return { ...result, calls, runs, repository };
  };
  return { run, repository };
}

const PLAIN_FILES = Object.fromEntries([
  'tests/cli.rs', 'tests/literals.rs', 'tests/nested/main.rs', 'src/lib.rs', 'src/main.rs',
  'src/project/semantic/resources_tests.rs', 'src/project/semantic/resources_tests/expression_projection.rs',
  'src/project/program/resolve/reuse_tests.rs',
].map(file => [file, '']));

/// A directory that is not a Git checkout.
function fixture(t) {
  const { run } = workspace(t, PLAIN_FILES);
  return (args, options) => run(args, options);
}

const build = (...targets) => ({ args: [...BUILD, ...targets], ...PROFILE });
const labels = result => result.runs.map(run => run.tool).sort();

/// Assert the calls, ignoring the working directory each one names.
function assertCalls(result, expected) {
  assert.deepEqual(result.calls.map(({ cwd, ...call }) => call), expected);
  for (const call of result.calls) assert.equal(call.cwd, result.repository);
}

test('an explicit integration-test file builds and runs only its target with pinned resources', t => {
  const result = fixture(t)(['tests/cli.rs']);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--test', 'cli')]);
  assert.deepEqual(result.runs.map(({ tool, args, cwd }) => ({ tool, args, cwd })),
    [{ tool: 'tests/cli.rs', args: RUN, cwd: result.repository }]);
  assert.equal(result.stdout, 'rust tests: passed, 0 skipped\n');
  assert.equal(result.stderr, '');
});

test('multiple explicit files build once and run exactly their integration-test targets', t => {
  const result = fixture(t)(['tests/cli.rs', 'tests/literals.rs']);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--test', 'cli', '--test', 'literals')]);
  assert.deepEqual(labels(result), ['tests/cli.rs', 'tests/literals.rs']);
});

test('the library root runs only library unit tests, without doc tests', t => {
  const result = fixture(t)(['src/lib.rs']);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--lib')]);
  assert.deepEqual(result.runs.map(({ tool, args }) => ({ tool, args })), [{ tool: 'src/lib.rs', args: RUN }]);
  assert.equal(result.stdout, 'rust tests: passed, 0 skipped\n');
  assert.equal(result.stderr, '');
});

test('library and integration roots select exactly those Cargo targets', t => {
  const result = fixture(t)(['src/lib.rs', 'tests/cli.rs']);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--lib', '--test', 'cli')]);
  assert.deepEqual(labels(result), ['src/lib.rs', 'tests/cli.rs']);
});

test('the binary root selects CLI unit tests with the fixed resource profile', t => {
  const result = fixture(t)(['src/main.rs']);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--bin', 'tsparse')]);
  assert.deepEqual(labels(result), ['src/main.rs']);
  assert.equal(result.stdout, 'rust tests: passed, 0 skipped\n');
});

test('binary, library and integration scopes select exactly those targets', t => {
  const result = fixture(t)(['src/main.rs', 'src/lib.rs', 'tests/cli.rs']);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--bin', 'tsparse', '--lib', '--test', 'cli')]);
  assert.deepEqual(labels(result), ['src/lib.rs', 'src/main.rs', 'tests/cli.rs']);
});

test('outside a Git checkout the changed wrapper runs every target, doc tests included', t => {
  const result = fixture(t)([]);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build(), { args: DOC, ...PROFILE }]);
  assert.deepEqual(labels(result), ['src/lib.rs', 'src/main.rs', 'tests/cli.rs', 'tests/literals.rs']);
  assert.equal(result.stdout,
    'rust tests: every target (the changes since main could not be determined)\nrust tests: passed, 0 skipped\n');
  assert.equal(result.stderr, '');
});

test('test executables run in parallel', t => {
  const result = fixture(t)(['src/lib.rs', 'src/main.rs', 'tests/cli.rs', 'tests/literals.rs'], { sleep: 500 });
  assert.equal(result.status, 0, result.stderr);
  let overlap = 0;
  for (const run of result.runs) {
    overlap = Math.max(overlap, result.runs.filter(other => other.started < run.ended && run.started < other.ended).length);
  }
  assert.ok(overlap >= 2, `at most ${overlap} executable ran at once`);
});

test('a test that cannot run on this platform is reported as a skip', t => {
  const result = fixture(t)(['tests/cli.rs'], { skip: 'SKIP case_policy: case-sensitive file system' });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout,
    'rust tests: passed, 1 skipped on this platform:\nSKIP case_policy: case-sensitive file system in tests/cli.rs\n');
});

test('skips from every executable are reported together', t => {
  const result = fixture(t)(['tests/cli.rs', 'tests/literals.rs'], { skip: 'SKIP case_policy: reason' });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout,
    'rust tests: passed, 2 skipped on this platform:\nSKIP case_policy: reason in tests/cli.rs\nSKIP case_policy: reason in tests/literals.rs\n');
});

test('invalid or unsupported paths fail before Cargo runs instead of widening scope', t => {
  const run = fixture(t);
  for (const args of [
    ['tests'], ['src/unknown.rs'], ['tests/missing.rs'], ['tests/nested/main.rs'],
    ['tests/*.rs'], ['--all'], ['../tests/cli.rs'], ['/tests/cli.rs'],
    ['tests/cli.rs', 'tests/missing.rs'],
  ]) {
    const result = run(args);
    assert.equal(result.status, 2, `${args}: ${result.stderr}`);
    assert.deepEqual(result.calls, [], String(args));
    assert.deepEqual(result.runs, [], String(args));
    assert.equal(result.stdout, '');
    assert.match(result.stderr, /expected .*tests\/<target>\.rs/);
  }
});

test('a failed build preserves its exit code and diagnostics and runs nothing', t => {
  const result = fixture(t)(['tests/cli.rs'], { buildExit: '3' });
  assert.equal(result.status, 3);
  assert.equal(result.calls.length, 1);
  assert.deepEqual(result.runs, []);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, /cargo stderr diagnostic/);
});

test('a failed test executable fails the run and replays its complete output', t => {
  const result = fixture(t)(['tests/cli.rs', 'tests/literals.rs'], { fail: 'tests/literals.rs' });
  assert.equal(result.status, 101);
  assert.deepEqual(labels(result), ['tests/cli.rs', 'tests/literals.rs']);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, /rust tests: 1 of 2 test executables failed: tests\/literals\.rs/);
  assert.match(result.stderr, /test failing_case \.\.\. FAILED/);
  assert.match(result.stderr, /tests\/literals\.rs failing diagnostic/);
  assert.doesNotMatch(result.stderr, /tests\/cli\.rs output/);
});

test('the semantic resource test file selects only its module with fixed resources', t => {
  const result = fixture(t)(['src/project/semantic/resources_tests.rs']);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--lib')]);
  assert.deepEqual(result.runs.map(({ tool, args }) => ({ tool, args })),
    [{ tool: 'src/lib.rs', args: [...RUN, 'project::semantic::resources_tests::'] }]);
  assert.equal(result.stdout, 'rust tests: passed, 0 skipped\n');
  assert.equal(result.stderr, '');
});

test('resource filtering cannot silently filter another selected target', t => {
  const run = fixture(t);
  for (const args of [
    ['src/project/semantic/resources_tests.rs', 'src/lib.rs'],
    ['tests/cli.rs', 'src/project/semantic/resources_tests.rs'],
    ['src/project/semantic/resources_tests.rs', 'src/project/semantic/resources_tests.rs'],
  ]) {
    const result = run(args);
    assert.equal(result.status, 2, result.stderr);
    assert.deepEqual(result.calls, []);
  }
});

test('a resource module with no executed tests fails and retains diagnostics', t => {
  const result = fixture(t)(['src/project/semantic/resources_tests.rs'], { passed: 0 });
  assert.equal(result.status, 1, result.stderr);
  assert.equal(result.calls.length, 1);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, /no tests executed/);
  assert.match(result.stderr, /src\/lib\.rs output/);
  assert.match(result.stderr, /0 passed/);
});

test('a missing resource test file is rejected before Cargo runs', t => {
  const { run, repository } = workspace(t, PLAIN_FILES);
  fs.rmSync(path.join(repository, 'src/project/semantic/resources_tests.rs'));
  const result = run(['src/project/semantic/resources_tests.rs']);
  assert.equal(result.status, 2, result.stderr);
  assert.deepEqual(result.calls, []);
});

test('one resource child module selects only its exact module with fixed resources', t => {
  const result = fixture(t)(['src/project/semantic/resources_tests/expression_projection.rs']);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--lib')]);
  assert.deepEqual(result.runs.map(({ tool, args }) => ({ tool, args })),
    [{ tool: 'src/lib.rs', args: [...RUN, 'project::semantic::resources_tests::expression_projection::'] }]);
});

test('resource child modules reject mixed, missing and non-module paths', t => {
  const run = fixture(t);
  for (const args of [
    ['src/project/semantic/resources_tests/expression_projection.rs', 'src/lib.rs'],
    ['src/project/semantic/resources_tests/expression_projection.rs', 'src/project/semantic/resources_tests.rs'],
    ['src/project/semantic/resources_tests/expression_projection.rs', 'src/project/semantic/resources_tests/expression_projection.rs'],
    ['src/project/semantic/resources_tests/missing.rs'],
    ['src/project/semantic/resources_tests/nested/expression_projection.rs'],
    ['src/project/semantic/resources_tests/*.rs'],
  ]) {
    const result = run(args);
    assert.equal(result.status, 2, result.stderr);
    assert.deepEqual(result.calls, []);
  }
});

test('an existing child file with no executed Rust tests cannot pass', t => {
  const result = fixture(t)(['src/project/semantic/resources_tests/expression_projection.rs'], { passed: 0 });
  assert.equal(result.status, 1, result.stderr);
  assert.equal(result.calls.length, 1);
  assert.match(result.stderr, /no tests executed/);
  assert.match(result.stderr, /src\/lib\.rs output/);
});

test('resolution reuse tests select only their module with fixed resources', t => {
  const result = fixture(t)(['src/project/program/resolve/reuse_tests.rs']);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--lib')]);
  assert.deepEqual(result.runs.map(({ tool, args }) => ({ tool, args })),
    [{ tool: 'src/lib.rs', args: [...RUN, 'project::program::resolve::reuse_tests::'] }]);
});

test('resolution reuse filtering rejects mixed or missing paths and empty execution', t => {
  const { run, repository } = workspace(t, PLAIN_FILES);
  const target = 'src/project/program/resolve/reuse_tests.rs';
  for (const args of [[target, 'src/lib.rs'], ['tests/cli.rs', target], [target, target]]) {
    const result = run(args);
    assert.equal(result.status, 2, result.stderr);
    assert.deepEqual(result.calls, []);
  }
  const empty = run([target], { passed: 0 });
  assert.equal(empty.status, 1, empty.stderr);
  assert.match(empty.stderr, /no tests executed/);
  assert.match(empty.stderr, /src\/lib\.rs output/);
  fs.rmSync(path.join(repository, target));
  const missing = run([target]);
  assert.equal(missing.status, 2, missing.stderr);
  assert.deepEqual(missing.calls, []);
});

test('the full wrapper runs every target, doc tests included', t => {
  const result = fixture(t)([], { wrapper: fullWrapper });
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build(), { args: DOC, ...PROFILE }]);
  assert.deepEqual(labels(result), ['src/lib.rs', 'src/main.rs', 'tests/cli.rs', 'tests/literals.rs']);
  assert.equal(result.stdout, 'rust tests: passed, 0 skipped\n');
});

// A Git checkout whose `main` holds a library, a binary, tests that do and
// do not run the binary, documents, tools, and fixtures.
const PROJECT = {
  '.gitignore': '/target/\n',
  'src/lib.rs': 'pub mod project;\n',
  'src/project.rs': '',
  'src/main.rs': 'mod cli;\nmod service;\nmod process_limits;\n',
  'src/cli/mod.rs': '',
  'src/service/mod.rs': '',
  'src/process_limits.rs': '',
  'tests/cli.rs': 'fn main() { let _ = env!("CARGO_BIN_EXE_tsparse"); }\n',
  'tests/literals.rs': '// library only\n',
  'tests/project_service.rs': 'mod support;\nuse support::service::Service;\n',
  'tests/cli_registry.rs': 'const REFERENCE: &str = "docs/cli-reference.md";\nfn main() { let _ = env!("CARGO_BIN_EXE_tsparse"); }\n',
  'tests/support/mod.rs': 'pub mod service;\n',
  'tests/support/service.rs': 'pub struct Service; // env!("CARGO_BIN_EXE_tsparse")\n',
  'docs/cli-reference.md': '# Reference\n',
  'tools/check.mjs': '',
  'fixtures/a.ts': '',
  'README.md': '# Project\n',
};

function git(directory, ...args) {
  const result = spawnSync('git', args, { cwd: directory, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout;
}

/// The project committed on `main`, with helpers to change it.
function repository(t) {
  const { run, repository } = workspace(t, PROJECT);
  git(repository, 'init', '-q', '-b', 'main');
  git(repository, '-c', 'user.email=test@example.com', '-c', 'user.name=Test', 'add', '-A');
  git(repository, '-c', 'user.email=test@example.com', '-c', 'user.name=Test', 'commit', '-qm', 'base');
  const write = (file, text) => {
    fs.mkdirSync(path.dirname(path.join(repository, file)), { recursive: true });
    fs.writeFileSync(path.join(repository, file), text);
  };
  const commit = () => {
    git(repository, 'add', '-A');
    git(repository, '-c', 'user.email=test@example.com', '-c', 'user.name=Test', 'commit', '-qm', 'change');
  };
  return { run, repository, write, commit };
}

const BINARY_TARGETS = ['--bin', 'tsparse', '--test', 'cli', '--test', 'cli_registry', '--test', 'project_service'];

test('a binary-crate change runs the binary unit tests and every test that runs the binary', t => {
  for (const file of ['src/cli/mod.rs', 'src/service/mod.rs', 'src/process_limits.rs', 'src/main.rs', 'src/cli/new.rs']) {
    const project = repository(t);
    project.write(file, '// changed\n');
    const result = project.run([]);
    assert.equal(result.status, 0, result.stderr);
    assertCalls(result, [build(...BINARY_TARGETS)]);
    assert.deepEqual(labels(result), ['src/main.rs', 'tests/cli.rs', 'tests/cli_registry.rs', 'tests/project_service.rs']);
    assert.equal(result.stdout,
      `rust tests: selected ${BINARY_TARGETS.join(' ')} (changes since main)\nrust tests: passed, 0 skipped\n`, file);
  }
});

test('a test-file change runs only that test, and a deleted one runs nothing', t => {
  const project = repository(t);
  project.write('tests/literals.rs', '// changed\n');
  project.write('tests/added.rs', '// new\n');
  const result = project.run([]);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--test', 'added', '--test', 'literals')]);

  const deleted = repository(t);
  fs.rmSync(path.join(deleted.repository, 'tests/literals.rs'));
  const none = deleted.run([]);
  assert.equal(none.status, 0, none.stderr);
  assert.deepEqual(none.calls, []);
  assert.equal(none.stdout, 'rust tests: no Rust test inputs changed since main\n');
});

test('a library, support, fixture, build, or unknown change runs every target', t => {
  for (const file of ['src/project.rs', 'src/new_module.rs', 'tests/support/mod.rs', 'tests/nested/main.rs',
    'fixtures/a.ts', 'Cargo.toml', 'build.rs', 'vendor/x/lib.rs', 'benches/b.rs', 'examples/e.rs', 'unknown.txt']) {
    const project = repository(t);
    project.write(file, '// changed\n');
    const result = project.run([]);
    assert.equal(result.status, 0, result.stderr);
    assertCalls(result, [build(), { args: DOC, ...PROFILE }]);
    assert.equal(result.stdout,
      `rust tests: every target (${file} changed since main)\nrust tests: passed, 0 skipped\n`, file);
  }
});

test('a document or tool change runs the tests that name its directory', t => {
  const project = repository(t);
  project.write('docs/cli-reference.md', '# Changed\n');
  const result = project.run([]);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--test', 'cli_registry')]);

  const binary = repository(t);
  binary.write('src/cli/mod.rs', 'const DOC: &str = "docs/cli-reference.md";\n');
  binary.commit();
  binary.write('docs/cli-reference.md', '# Changed\n');
  const both = binary.run([]);
  assert.equal(both.status, 0, both.stderr);
  assertCalls(both, [build('--bin', 'tsparse', '--test', 'cli_registry')]);
});

test('a change outside every Rust test input runs nothing', t => {
  const project = repository(t);
  project.write('tools/check.mjs', '// changed\n');
  project.write('README.md', '# Changed\n');
  const result = project.run([]);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.calls, []);
  assert.deepEqual(result.runs, []);
  assert.equal(result.stdout, 'rust tests: no Rust test inputs changed since main\n');
});

test('a binary helper outside the service support module makes a binary change run every target', t => {
  const project = repository(t);
  project.write('tests/support/mod.rs', 'pub mod service;\npub fn exe() -> &\'static str { env!("CARGO_BIN_EXE_tsparse") }\n');
  project.commit();
  project.write('src/cli/mod.rs', '// changed\n');
  const result = project.run([]);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build(), { args: DOC, ...PROFILE }]);
});

test('a binary module that the library also declares makes a binary change run every target', t => {
  const project = repository(t);
  project.write('src/lib.rs', 'pub mod project;\npub(crate) mod process_limits;\n');
  project.commit();
  project.write('src/process_limits.rs', '// changed\n');
  const result = project.run([]);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build(), { args: DOC, ...PROFILE }]);
});

test('a passing run is recorded, and the next run selects only what changed since it', t => {
  const project = repository(t);
  project.write('src/cli/mod.rs', '// changed\n');
  assert.equal(project.run([]).status, 0);
  project.write('tests/literals.rs', '// changed\n');
  const result = project.run([]);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--test', 'literals')]);
  assert.equal(result.stdout,
    'rust tests: selected --test literals (changes since the last passing run)\nrust tests: passed, 0 skipped\n');
  const unchanged = project.run([]);
  assert.deepEqual(unchanged.calls, []);
  assert.equal(unchanged.stdout, 'rust tests: no Rust test inputs changed since the last passing run\n');
});

test('a failing run is not recorded', t => {
  const project = repository(t);
  project.write('tests/literals.rs', '// changed\n');
  assert.equal(project.run([], { fail: 'tests/literals.rs' }).status, 101);
  const result = project.run([]);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--test', 'literals')]);
  assert.match(result.stdout, /changes since main/);
});

test('an explicit-path run is not recorded', t => {
  const project = repository(t);
  project.write('tests/literals.rs', '// changed\n');
  assert.equal(project.run(['tests/literals.rs']).status, 0);
  const result = project.run([]);
  assertCalls(result, [build('--test', 'literals')]);
  assert.match(result.stdout, /changes since main/);
});

test('a run whose inputs change while it runs is not recorded', t => {
  const project = repository(t);
  project.write('tests/literals.rs', '// changed\n');
  const touched = path.join(project.repository, 'tests/cli.rs');
  assert.equal(project.run([], { touch: touched }).status, 0);
  const result = project.run([]);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build('--test', 'cli', '--test', 'literals')]);
  assert.match(result.stdout, /changes since main/);
});

test('a passing full run is recorded for the changed wrapper', t => {
  const project = repository(t);
  project.write('src/project.rs', '// changed\n');
  assert.equal(project.run([], { wrapper: fullWrapper }).status, 0);
  project.write('tests/literals.rs', '// changed\n');
  const result = project.run([]);
  assertCalls(result, [build('--test', 'literals')]);
  assert.match(result.stdout, /changes since the last passing run/);
});

test('a recorded run that Git no longer holds falls back to the target branch', t => {
  const project = repository(t);
  fs.mkdirSync(path.join(project.repository, 'target/validation'), { recursive: true });
  fs.writeFileSync(path.join(project.repository, 'target/validation/rust-tests-passed.tree'), '0'.repeat(40) + '\nany\n');
  project.write('tests/literals.rs', '// changed\n');
  const result = project.run([]);
  assertCalls(result, [build('--test', 'literals')]);
  assert.match(result.stdout, /changes since main/);
});

test('a test file marked to run alone runs after the others, one at a time', t => {
  const { run } = workspace(t, {
    ...PLAIN_FILES,
    'tests/literals.rs': '// validation: run alone: it asserts elapsed time\n',
    'tests/timing.rs': '//! validation: run alone\n',
  });
  const result = run(['src/lib.rs', 'src/main.rs', 'tests/cli.rs', 'tests/literals.rs', 'tests/timing.rs'], { sleep: 300 });
  assert.equal(result.status, 0, result.stderr);
  const alone = result.runs.filter(entry => ['tests/literals.rs', 'tests/timing.rs'].includes(entry.tool));
  const together = result.runs.filter(entry => !alone.includes(entry));
  assert.equal(alone.length, 2);
  assert.equal(together.length, 3);
  const lastTogether = Math.max(...together.map(entry => entry.ended));
  for (const entry of alone) assert.ok(entry.started >= lastTogether, `${entry.tool} started before the others ended`);
  const [first, second] = alone.sort((a, b) => a.started - b.started);
  assert.ok(second.started >= first.ended, 'the run-alone executables overlapped');
});

test('a toolchain change since the recorded pass runs every target', t => {
  const project = repository(t);
  project.write('tests/literals.rs', '// changed\n');
  assert.equal(project.run([]).status, 0);
  const recorded = path.join(project.repository, 'target/validation/rust-tests-passed.tree');
  const [tree] = fs.readFileSync(recorded, 'utf8').split('\n');
  fs.writeFileSync(recorded, `${tree}\nanother toolchain\n`);
  project.write('tests/cli.rs', '// changed\n');
  const result = project.run([]);
  assert.equal(result.status, 0, result.stderr);
  assertCalls(result, [build(), { args: DOC, ...PROFILE }]);
  assert.equal(result.stdout,
    'rust tests: every target (the Rust toolchain changed since the last passing run)\nrust tests: passed, 0 skipped\n');
});

test('a build directory that Git does not ignore is not a change', t => {
  const project = repository(t);
  project.write('build/debug/deps/cli-0123', 'artifact\n');
  const result = project.run([], { targetDir: 'build' });
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.calls, []);
  assert.equal(result.stdout, 'rust tests: no Rust test inputs changed since main\n');
});

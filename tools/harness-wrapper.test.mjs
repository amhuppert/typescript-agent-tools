import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const wrappers = {
  full: fileURLToPath(new URL('../scripts/validate/harness-test-full.sh', import.meta.url)),
  changed: fileURLToPath(new URL('../scripts/validate/harness-test-changed.sh', import.meta.url)),
};
const platform = `${process.platform}-${process.arch}`;

// A canonical checkout with a linked worktree, as Command Center runs the
// wrappers. The worktree holds small harness tests; a `node` shim on PATH
// records each invocation before running the real Node.
function fixture(t) {
  // The prefix names no Rust tool, so a test that records this path is not
  // selected for Rust changes.
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'harness-wrapper-fixture-')));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const git = (...args) => {
    const result = spawnSync('git', ['-c', 'user.name=t', '-c', 'user.email=t@example.com', ...args], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
  };
  const canonical = path.join(directory, 'canonical');
  const worktree = path.join(directory, 'worktree');
  fs.mkdirSync(canonical);
  git('-C', canonical, 'init', '-q', '-b', 'main');
  git('-C', canonical, 'commit', '-q', '--allow-empty', '-m', 'initial');
  git('-C', canonical, 'worktree', 'add', '-q', worktree);
  const calls = path.join(directory, 'node-calls.jsonl');
  const seen = path.join(directory, 'compilers.json');
  const bin = path.join(directory, 'bin');
  fs.mkdirSync(bin);
  fs.writeFileSync(path.join(bin, 'node'), `#!${process.execPath}
require('node:fs').appendFileSync(${JSON.stringify(calls)}, JSON.stringify({ args: process.argv.slice(2), nodeOptions: process.env.NODE_OPTIONS }) + '\\n');
const result = require('node:child_process').spawnSync(process.execPath, process.argv.slice(2), { stdio: 'inherit' });
process.exit(result.status ?? 1);
`, { mode: 0o755 });

  const write = (file, text) => {
    fs.mkdirSync(path.dirname(path.join(worktree, file)), { recursive: true });
    fs.writeFileSync(path.join(worktree, file), text);
  };
  write('tools/pass.test.mjs', `import { test } from 'node:test';\ntest('passes', () => {});\n`);
  write('tools/compiler.test.mjs', `import fs from 'node:fs';
import { test } from 'node:test';
const { TYPE_SCRIPT_API: api, TYPE_SCRIPT_SEMANTIC_API: semantic, TYPE_SCRIPT_NATIVE: native, TYPE_SCRIPT_56_API: ts56 } = process.env;
fs.writeFileSync(${JSON.stringify(seen)}, JSON.stringify({ api, semantic, native, ts56 }));
test('needs the syntax API', { skip: !api && 'TYPE_SCRIPT_API is not set' }, () => {});
`);
  write('tools/helper.mjs', `throw new Error('a non-test module ran');\n`);
  write('tools/nested/inner.test.mjs', `import { test } from 'node:test';\ntest('nested', () => {});\n`);
  write('.gitignore', '/target/\n');

  const install = file => {
    const target = path.join(canonical, '.cache/compiler-oracles', file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, '', { mode: 0o755 });
    return target;
  };
  const installAll = () => ({
    api: install('typescript-6.0.3/package/lib/typescript.js'),
    semantic: path.dirname(install('typescript-7.0.2/package/package.json')),
    native: install(`typescript-${platform}-7.0.2/package/lib/tsc`),
    ts56: path.dirname(install('typescript-5.6.3/package/package.json')),
  });

  const run = (wrapper, args = [], environment = {}) => {
    fs.writeFileSync(calls, '');
    fs.rmSync(seen, { force: true });
    const env = { ...process.env, PATH: `${bin}${path.delimiter}${process.env.PATH}`, NODE_OPTIONS: '--max-old-space-size=9999' };
    for (const name of ['TYPE_SCRIPT_API', 'TYPE_SCRIPT_SEMANTIC_API', 'TYPE_SCRIPT_NATIVE', 'TYPE_SCRIPT_56_API', 'HARNESS_REQUIRE_COMPILERS', 'NODE_TEST_CONTEXT'])
      delete env[name];
    Object.assign(env, environment);
    const result = spawnSync(wrappers[wrapper], args, { cwd: worktree, env, encoding: 'utf8', timeout: 60_000 });
    assert.equal(result.error, undefined);
    assert.equal(result.signal, null, result.stderr);
    const testRuns = fs.readFileSync(calls, 'utf8').split('\n').filter(Boolean).map(line => JSON.parse(line))
      .filter(call => call.args[0] === '--test');
    const compilers = fs.existsSync(seen) ? JSON.parse(fs.readFileSync(seen, 'utf8')) : undefined;
    return { ...result, testRuns, compilers };
  };
  const files = call => call.args.slice(call.args.indexOf('--') + 1);
  const option = (call, name) => call.args.find(arg => arg.startsWith(`--${name}=`))?.slice(name.length + 3);
  // Commit the worktree, so a changed selection compares against it.
  const commit = () => {
    git('-C', worktree, 'add', '-A');
    git('-C', worktree, 'commit', '-q', '-m', 'base');
  };
  const branch = spawnSync('git', ['-C', worktree, 'rev-parse', '--abbrev-ref', 'HEAD'], { encoding: 'utf8' }).stdout.trim();
  return { write, install, installAll, run, files, option, commit, branch, remove: file => fs.rmSync(path.join(worktree, file)) };
}

test('explicit harness test files run exactly those files on at most two workers', t => {
  const { write, run, files, option } = fixture(t);
  const one = run('changed', ['tools/pass.test.mjs']);
  assert.equal(one.status, 0, one.stderr);
  assert.equal(one.testRuns.length, 1);
  assert.deepEqual(files(one.testRuns[0]), ['tools/pass.test.mjs']);
  assert.equal(option(one.testRuns[0], 'test-concurrency'), '1');
  assert.equal(one.testRuns[0].nodeOptions, '--max-old-space-size=2048');
  write('tools/other.test.mjs', `import { test } from 'node:test';\ntest('other', () => {});\n`);
  const three = run('changed', ['tools/pass.test.mjs', 'tools/compiler.test.mjs', 'tools/other.test.mjs']);
  assert.equal(three.status, 0, three.stderr);
  assert.deepEqual(files(three.testRuns[0]), ['tools/pass.test.mjs', 'tools/compiler.test.mjs', 'tools/other.test.mjs']);
  assert.equal(option(three.testRuns[0], 'test-concurrency'), '2');
});

test('without paths the changed wrapper runs the whole harness suite', t => {
  const { run, files, option } = fixture(t);
  const result = run('changed');
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(files(result.testRuns[0]), ['tools/compiler.test.mjs', 'tools/pass.test.mjs']);
  assert.equal(option(result.testRuns[0], 'test-concurrency'), '2');
});

test('paths other than existing tools/*.test.mjs files fail with exit 2 before any test runs', t => {
  const { run } = fixture(t);
  for (const args of [
    ['tools/helper.mjs'], ['tools/missing.test.mjs'], ['tools'], ['tools/*.test.mjs'], ['--all'],
    ['../tools/pass.test.mjs'], ['/tools/pass.test.mjs'], ['./tools/pass.test.mjs'],
    ['tools/nested/inner.test.mjs'], ['tests/cli.rs'], ['tools/pass.test.mjs', 'tools/helper.mjs'],
  ]) {
    const result = run('changed', args);
    assert.equal(result.status, 2, `${args}: ${result.stderr}`);
    assert.deepEqual(result.testRuns, [], String(args));
    assert.equal(result.stdout, '');
    assert.match(result.stderr, /expected an existing harness test file tools\/<name>\.test\.mjs/);
  }
});

test('success prints one line of counts, including skips, and nothing else', t => {
  for (const wrapper of ['full', 'changed']) {
    const { run } = fixture(t);
    const result = run(wrapper);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout,
      'harness tests: 2 total, 1 executed, 1 passed, 1 skipped (HARNESS_REQUIRE_COMPILERS=1 fails on skips)\n');
    assert.equal(result.stderr, '');
  }
});

test('compilers in the canonical checkout cache reach the tests; explicit variables win', t => {
  const { installAll, run } = fixture(t);
  const cached = installAll();
  const result = run('full');
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, 'harness tests: 2 total, 2 executed, 2 passed, 0 skipped\n');
  assert.deepEqual(result.compilers, cached);
  const explicit = run('full', [], { TYPE_SCRIPT_API: '/explicit/typescript.js', TYPE_SCRIPT_NATIVE: '/explicit/tsc' });
  assert.equal(explicit.status, 0, explicit.stderr);
  assert.deepEqual(explicit.compilers, { ...cached, api: '/explicit/typescript.js', native: '/explicit/tsc' });
});

test('strict mode needs every compiler and fails with exit 2 before running tests otherwise', t => {
  const { install, run } = fixture(t);
  const none = run('full', [], { HARNESS_REQUIRE_COMPILERS: '1' });
  assert.equal(none.status, 2);
  assert.equal(none.stdout, '');
  assert.match(none.stderr, /requires every compiler; no tests ran/);
  assert.match(none.stderr, /no syntax API at .*typescript-6\.0\.3\/package\/lib\/typescript\.js/);
  assert.match(none.stderr, /no semantic API package at .*typescript-7\.0\.2\/package/);
  assert.match(none.stderr, /no TypeScript 5\.6\.3 package at .*typescript-5\.6\.3\/package/);
  assert.deepEqual(none.testRuns, []);
  install('typescript-6.0.3/package/lib/typescript.js');
  install('typescript-7.0.2/package/package.json');
  const partial = run('changed', ['tools/pass.test.mjs'], { HARNESS_REQUIRE_COMPILERS: '1' });
  assert.equal(partial.status, 2);
  assert.doesNotMatch(partial.stderr, /syntax API/);
  assert.match(partial.stderr, /no native compiler at/);
  assert.deepEqual(partial.testRuns, []);
  install(`typescript-${platform}-7.0.2/package/lib/tsc`);
  const without56 = run('changed', ['tools/pass.test.mjs'], { HARNESS_REQUIRE_COMPILERS: '1' });
  assert.equal(without56.status, 2);
  assert.doesNotMatch(without56.stderr, /native compiler|semantic API|syntax API/);
  assert.match(without56.stderr, /no TypeScript 5\.6\.3 package at .*typescript-5\.6\.3\/package/);
  assert.deepEqual(without56.testRuns, []);
  const typo = run('full', [], { HARNESS_REQUIRE_COMPILERS: 'yes' });
  assert.equal(typo.status, 2);
  assert.match(typo.stderr, /HARNESS_REQUIRE_COMPILERS must be 1, 0, or unset/);
  assert.deepEqual(typo.testRuns, []);
});

test('strict mode fails on any skipped test, naming it, and passes when nothing skips', t => {
  const { installAll, write, remove, run } = fixture(t);
  installAll();
  write('tools/platform.test.mjs', `import { test } from 'node:test';\ntest('platform only', { skip: 'not on this platform' }, () => {});\n`);
  const skipped = run('full', [], { HARNESS_REQUIRE_COMPILERS: '1' });
  assert.equal(skipped.status, 1);
  assert.equal(skipped.stdout, '');
  assert.match(skipped.stderr, /^harness tests: 3 total, 2 executed, 2 passed, 1 skipped; strict mode/);
  assert.match(skipped.stderr, /skipped: tools\/platform\.test\.mjs: platform only \(not on this platform\)/);
  const lenient = run('full');
  assert.equal(lenient.status, 0, lenient.stderr);
  assert.match(lenient.stdout, /1 skipped \(HARNESS_REQUIRE_COMPILERS=1 fails on skips\)/);
  remove('tools/platform.test.mjs');
  const complete = run('full', [], { HARNESS_REQUIRE_COMPILERS: '1' });
  assert.equal(complete.status, 0, complete.stderr);
  assert.equal(complete.stdout, 'harness tests: 2 total, 2 executed, 2 passed, 0 skipped\n');
});

test('a failing test keeps the runner exit code and replays the complete output', t => {
  const { write, run } = fixture(t);
  write('tools/fail.test.mjs', `import assert from 'node:assert/strict';
import { test } from 'node:test';
test('fails', () => { console.log('diagnostic output'); assert.equal(1, 2, 'deliberate failure marker'); });
`);
  const result = run('full');
  assert.equal(result.status, 1);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, /diagnostic output/);
  assert.match(result.stderr, /deliberate failure marker/);
  assert.match(result.stderr, /ℹ fail 1/);
});

test('a run in which every selected test skips fails instead of passing', t => {
  const { run } = fixture(t);
  const result = run('changed', ['tools/compiler.test.mjs']);
  assert.equal(result.status, 1);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, /0 executed.*no test executed/);
  assert.match(result.stderr, /skipped: tools\/compiler\.test\.mjs: needs the syntax API \(TYPE_SCRIPT_API is not set\)/);
});

// A committed worktree whose tests reach a module, the Rust tool, or the
// validation scripts.
function selection(t) {
  const project = fixture(t);
  project.write('tools/lib.mjs', 'export const value = 1;\n');
  project.write('tools/reporter.mjs', 'export default 1;\n');
  project.write('tools/uses-lib.test.mjs', `import { test } from 'node:test';\nimport { value } from './lib.mjs';\ntest('lib', () => value);\n`);
  project.write('tools/rust.test.mjs', `import { test } from 'node:test';\nconst binary = 'target/debug/tsparse';\ntest('rust', () => binary);\n`);
  project.write('tools/wrapper.test.mjs', `import { test } from 'node:test';\nconst script = new URL('../scripts/validate/x.sh', import.meta.url);\ntest('wrapper', () => script);\n`);
  project.write('scripts/validate/x.sh', 'node tools/reporter.mjs\n');
  project.write('src/main.rs', '');
  project.write('fixtures/case.ts', '');
  project.write('docs/notes.md', '');
  project.commit();
  const changed = (environment = {}) => project.run('changed', [], { TARGET_BRANCH: project.branch, ...environment });
  return { ...project, changed };
}

const ALL = ['tools/compiler.test.mjs', 'tools/pass.test.mjs', 'tools/rust.test.mjs', 'tools/uses-lib.test.mjs', 'tools/wrapper.test.mjs'];

test('without paths each changed file selects the harness tests that can observe it', t => {
  for (const [file, selected] of [
    ['tools/pass.test.mjs', ['tools/pass.test.mjs']],
    ['tools/lib.mjs', ['tools/uses-lib.test.mjs']],
    ['tools/reporter.mjs', ['tools/wrapper.test.mjs']],
    ['src/main.rs', ['tools/rust.test.mjs']],
    ['scripts/validate/x.sh', ['tools/wrapper.test.mjs']],
  ]) {
    const { write, changed, files, option, branch } = selection(t);
    const contents = {
      'tools/pass.test.mjs': `import { test } from 'node:test';\ntest('changed', () => {});\n`,
      'tools/lib.mjs': 'export const value = 2;\n',
      'tools/reporter.mjs': 'export default 2;\n',
    };
    write(file, contents[file] ?? '// changed\n');
    const result = changed();
    assert.equal(result.status, 0, `${file}: ${result.stderr}`);
    assert.equal(result.testRuns.length, 1, `${file}: ${result.stdout}${result.stderr}`);
    assert.deepEqual(files(result.testRuns[0]), selected, file);
    assert.equal(option(result.testRuns[0], 'test-concurrency'), '1', file);
    assert.match(result.stdout, new RegExp(`^harness tests: selected 1 of 5 files \\(changes since ${branch}\\)\n`), file);
  }
});

test('without paths a package or unknown change runs every harness test', t => {
  for (const file of ['package.json', 'unknown.txt']) {
    const { write, changed, files } = selection(t);
    write(file, '// changed\n');
    const result = changed();
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(files(result.testRuns[0]), ALL, file);
    assert.match(result.stdout, /^harness tests: \d+ total/, file);
  }
});

test('without paths a change no harness test can observe runs nothing', t => {
  // Documents no module imports: docs, the agent plugins and their generated
  // packages, and top-level Markdown such as the shared agents block.
  for (const file of ['docs/notes.md', 'plugins/tool/skills/tool/SKILL.md', 'SHARED_AGENTS_BLOCK.md']) {
    const { write, changed, branch } = selection(t);
    write(file, 'changed\n');
    const result = changed();
    assert.equal(result.status, 0, `${file}: ${result.stderr}`);
    assert.deepEqual(result.testRuns, [], file);
    assert.equal(result.stdout, `harness tests: no harness test inputs changed since ${branch}\n`, file);
  }
});

test('a passing harness run is recorded, and the next one selects only what changed since it', t => {
  const { write, changed, files } = selection(t);
  write('tools/lib.mjs', 'export const value = 2;\n');
  assert.equal(changed().status, 0);
  write('src/main.rs', '// changed\n');
  const result = changed();
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(files(result.testRuns[0]), ['tools/rust.test.mjs']);
  assert.match(result.stdout, /\(changes since the last passing run\)/);
  const again = changed();
  assert.deepEqual(again.testRuns, []);
  assert.equal(again.stdout, 'harness tests: no harness test inputs changed since the last passing run\n');
});

test('a failing harness run is not recorded', t => {
  const { write, changed, files, branch } = selection(t);
  write('tools/pass.test.mjs', `import assert from 'node:assert/strict';\nimport { test } from 'node:test';\ntest('fails', () => assert.fail('marker'));\n`);
  assert.equal(changed().status, 1);
  write('tools/pass.test.mjs', `import { test } from 'node:test';\ntest('passes after the fix', () => {});\n`);
  const result = changed();
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(files(result.testRuns[0]), ['tools/pass.test.mjs']);
  assert.match(result.stdout, new RegExp(`\\(changes since ${branch}\\)`));
});

test('a compiler change since the recorded pass runs every harness test', t => {
  const { write, changed, files, installAll } = selection(t);
  write('tools/lib.mjs', 'export const value = 2;\n');
  assert.equal(changed().status, 0);
  installAll();
  write('tools/pass.test.mjs', `import { test } from 'node:test';\ntest('passes again', () => {});\n`);
  const result = changed();
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(files(result.testRuns[0]), ALL);
});

test('a passing full harness run is recorded for the changed wrapper', t => {
  const { write, changed, files, run, branch } = selection(t);
  write('tools/lib.mjs', 'export const value = 2;\n');
  assert.equal(run('full', [], { TARGET_BRANCH: branch }).status, 0);
  write('src/main.rs', '// changed\n');
  const result = changed();
  assert.deepEqual(files(result.testRuns[0]), ['tools/rust.test.mjs']);
  assert.match(result.stdout, /\(changes since the last passing run\)/);
});

// A committed worktree whose tests reach fixtures, Rust sources, and vendor/
// through each path form a test can name.
function readers(t) {
  const project = fixture(t);
  const files = {
    'fixtures/cases/one/a.ts': '', 'fixtures/cases/one/tsconfig.json': '{}',
    'fixtures/cases/two/b.ts': '', 'fixtures/cases/two/tsconfig.json': '{}',
    'fixtures/other/c.ts': '', 'fixtures/top.json': '{}',
    'src/lib.rs': '', 'src/other.rs': '', 'vendor/lib.d.ts': '',
    'plugins/tool/skills/tool/SKILL.md': '', 'SHARED_AGENTS_BLOCK.md': '',
    'tools/one.test.mjs': `import { test } from 'node:test';\nconst root = new URL('../fixtures/cases/one/', import.meta.url);\ntest('one', () => root);\n`,
    'tools/each.test.mjs': `import { test } from 'node:test';\nconst each = name => new URL(\`../fixtures/cases/\${name}/\`, import.meta.url);\ntest('each', () => each);\n`,
    'tools/project.test.mjs': `import { test } from 'node:test';\nconst config = '../fixtures/cases/two/tsconfig.json';\ntest('project', () => config);\n`,
    'tools/joined.test.mjs': `import path from 'node:path';\nimport { test } from 'node:test';\nconst root = path.join('..', 'fixtures');\ntest('joined', () => root);\n`,
    'tools/top.test.mjs': `import { test } from 'node:test';\nconst top = '../fixtures/top.json';\ntest('top', () => top);\n`,
    'tools/source.test.mjs': `import { test } from 'node:test';\nconst source = '../src/lib.rs';\ntest('source', () => source);\n`,
    'tools/vendor.test.mjs': `import { test } from 'node:test';\nconst libraries = '../vendor/';\ntest('vendor', () => libraries);\n`,
    'tools/skill.test.mjs': `import { test } from 'node:test';\nconst skill = '../plugins/tool/skills/tool/SKILL.md';\ntest('skill', () => skill);\n`,
    'tools/block.test.mjs': `import { test } from 'node:test';\nconst block = '../SHARED_AGENTS_BLOCK.md';\ntest('block', () => block);\n`,
  };
  for (const [file, text] of Object.entries(files)) project.write(file, text);
  project.commit();
  const changed = () => project.run('changed', [], { TARGET_BRANCH: project.branch });
  return { ...project, changed };
}

test('without paths a plugin or top-level document change runs the harness tests that name it', t => {
  for (const [file, selected] of [
    ['plugins/tool/skills/tool/SKILL.md', ['tools/skill.test.mjs']],
    ['SHARED_AGENTS_BLOCK.md', ['tools/block.test.mjs']],
  ]) {
    const { write, changed, files } = readers(t);
    write(file, 'changed\n');
    const result = changed();
    assert.equal(result.status, 0, `${file}: ${result.stderr}`);
    assert.equal(result.testRuns.length, 1, `${file}: ${result.stdout}${result.stderr}`);
    assert.deepEqual(files(result.testRuns[0]), selected, file);
  }
});

test('without paths a fixture change runs the harness tests that name a path reaching it', t => {
  for (const [file, selected] of [
    ['fixtures/cases/one/a.ts', ['tools/each.test.mjs', 'tools/joined.test.mjs', 'tools/one.test.mjs']],
    ['fixtures/cases/two/b.ts', ['tools/each.test.mjs', 'tools/joined.test.mjs', 'tools/project.test.mjs']],
    ['fixtures/other/c.ts', ['tools/joined.test.mjs']],
    ['fixtures/top.json', ['tools/joined.test.mjs', 'tools/top.test.mjs']],
  ]) {
    const { write, changed, files } = readers(t);
    write(file, '// changed\n');
    const result = changed();
    assert.equal(result.status, 0, `${file}: ${result.stderr}`);
    assert.equal(result.testRuns.length, 1, `${file}: ${result.stdout}${result.stderr}`);
    assert.deepEqual(files(result.testRuns[0]), selected, file);
  }
});

test('without paths a Rust change runs the tests that run the tool or name the changed file', t => {
  for (const [file, selected] of [
    ['src/lib.rs', ['tools/source.test.mjs']],
    ['src/other.rs', []],
    ['vendor/lib.d.ts', ['tools/vendor.test.mjs']],
  ]) {
    const { write, changed, files } = readers(t);
    write(file, '// changed\n');
    const result = changed();
    assert.equal(result.status, 0, `${file}: ${result.stderr}`);
    if (selected.length === 0) {
      assert.deepEqual(result.testRuns, [], file);
      assert.match(result.stdout, /no harness test inputs changed/, file);
    } else {
      assert.deepEqual(files(result.testRuns[0]), selected, file);
    }
  }
});

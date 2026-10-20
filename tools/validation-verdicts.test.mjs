import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

// The full format and lint checks say `format: ok` and `lint: ok` when
// clean, from run_quiet given a label (scripts/validate/common.sh), and keep
// a failure's complete output and exit status. A `cargo` shim on PATH stands
// in for rustfmt and clippy, recording its arguments.
const scripts = fileURLToPath(new URL('../scripts/validate/', import.meta.url));

function fixture(t) {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'validation-verdicts-')));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const bin = path.join(directory, 'bin');
  const calls = path.join(directory, 'calls');
  fs.mkdirSync(bin);
  fs.writeFileSync(path.join(bin, 'cargo'), `#!/bin/sh
printf '%s\\n' "$*" >>"${calls}"
if [ -n "\${SHIM_FAIL:-}" ]; then
  printf 'Diff in src/lib.rs at line 1\\n'
  printf 'warning: unused\\n' >&2
  exit 3
fi
printf 'noise a clean run hides\\n'
`, { mode: 0o755 });
  const run = (command, environment = {}) => {
    fs.rmSync(calls, { force: true });
    const result = spawnSync('bash', ['-c', command], {
      cwd: directory, encoding: 'utf8', timeout: 30_000,
      env: { ...process.env, PATH: `${bin}${path.delimiter}${process.env.PATH}`, ...environment },
    });
    assert.equal(result.error, undefined);
    const recorded = fs.existsSync(calls) ? fs.readFileSync(calls, 'utf8').trim().split('\n') : [];
    return { status: result.status, stdout: result.stdout, stderr: result.stderr, calls: recorded };
  };
  return { run };
}

for (const [script, label, args] of [
  ['format-full.sh', 'format', 'fmt --all -- --check --color never'],
  ['lint-full.sh', 'lint', 'clippy --offline --jobs 2 --all-targets --no-deps -- -D warnings'],
]) {
  test(`${script} prints ${label}: ok when clean and keeps a failure's output`, t => {
    const { run } = fixture(t);
    const clean = run(`'${path.join(scripts, script)}'`);
    assert.deepEqual(clean, { status: 0, stdout: `${label}: ok\n`, stderr: '', calls: [args] });
    const failed = run(`'${path.join(scripts, script)}'`, { SHIM_FAIL: '1' });
    assert.deepEqual(failed, {
      status: 3, stdout: '', stderr: 'Diff in src/lib.rs at line 1\nwarning: unused\n', calls: [args],
    });
  });
}

test('run_quiet without a label stays silent when the command succeeds', t => {
  const { run } = fixture(t);
  const common = path.join(scripts, 'common.sh');
  assert.deepEqual(run(`source '${common}'; run_quiet cargo build`),
    { status: 0, stdout: '', stderr: '', calls: ['build'] });
  assert.deepEqual(run(`source '${common}'; run_quiet --label build cargo build`),
    { status: 0, stdout: 'build: ok\n', stderr: '', calls: ['build'] });
  assert.deepEqual(run(`source '${common}'; run_quiet --label build cargo build`, { SHIM_FAIL: '1' }),
    { status: 3, stdout: '', stderr: 'Diff in src/lib.rs at line 1\nwarning: unused\n', calls: ['build'] });
});

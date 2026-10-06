import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { runParser, parserFailure } from './parser-process.mjs';

// Real-compiler tests are skipped, with this reason, unless the API is configured.
const apiUnavailable = !process.env.TYPE_SCRIPT_API && 'TYPE_SCRIPT_API is not set';

test('only a normal exit 1 is a syntax rejection', () => {
  for (const [code, outcome] of [[0, 'accepted'], [1, 'rejected'], [2, 'failed'], [3, 'failed']]) {
    const result = runParser(process.execPath, ['-e', `process.exit(${code})`]);
    assert.equal(result.outcome, outcome);
    assert.equal(result.status, code);
  }
});

test('crashes, timeouts, output overflow, and launch errors are process failures', () => {
  const results = [
    runParser(process.execPath, ['-e', 'process.kill(process.pid, "SIGKILL")']),
    runParser(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { timeout: 100 }),
    runParser(process.execPath, ['-e', 'process.stdout.write("x".repeat(1000000))'], { maxBuffer: 32 }),
    runParser(path.join(os.tmpdir(), 'tsparse-missing-binary-' + process.pid), []),
  ];
  for (const result of results) {
    assert.equal(result.outcome, 'failed');
    assert.ok(parserFailure(result).error);
  }
});

test('real compiler API: an invalid upstream fixture cannot pass on exit 2 or a launch error', { skip: apiUnavailable }, t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-runner-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const fixture = path.join(directory, 'upstream/tests/cases/compiler');
  fs.mkdirSync(fixture, { recursive: true });
  fs.writeFileSync(path.join(fixture, 'invalid.ts'), 'const value = ;');
  const runner = fileURLToPath(new URL('./upstream-corpus.mjs', import.meta.url));
  const binary = path.join(directory, 'parser');
  for (const [name, exit, passed] of [['syntax-rejection', 1, 1], ['io-failure', 2, 0], ['launch-error', null, 0]]) {
    if (exit === null) fs.unlinkSync(binary);
    else fs.writeFileSync(binary, `#!/bin/sh\nexit ${exit}\n`, { mode: 0o755 });
    const result = spawnSync(process.execPath, [runner, path.join(directory, 'upstream')], {
      cwd: directory, env: { ...process.env, TSPARSE: binary }, encoding: 'utf8', timeout: 10_000,
    });
    assert.equal(result.error, undefined, result.stderr);
    assert.equal(result.status, passed ? 0 : 1, result.stdout + result.stderr);
    const report = JSON.parse(fs.readFileSync(path.join(directory, '.cache/upstream-corpus.json'), 'utf8'));
    assert.equal(report.invalid, 1, name);
    assert.equal(report.passed, passed, name);
    assert.equal(report.failures.length, passed ? 0 : 1, name);
  }
});

test('real compiler API: grammar runner reports exit 2 on compiler-invalid inputs', { skip: apiUnavailable }, t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-grammar-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const binary = path.join(directory, 'parser');
  fs.writeFileSync(binary, '#!/bin/sh\nexit 2\n', { mode: 0o755 });
  const runner = fileURLToPath(new URL('./grammar-corpus.mjs', import.meta.url));
  const result = spawnSync(process.execPath, [runner], { cwd: directory,
    env: { ...process.env, TSPARSE: binary }, encoding: 'utf8', timeout: 30_000 });
  assert.equal(result.status, 1, result.stderr);
  const report = JSON.parse(fs.readFileSync(path.join(directory, '.cache/grammar-corpus.json'), 'utf8'));
  assert.ok(report.invalid > 0);
  assert.equal(report.passed, 0);
  assert.equal(report.failures.length, report.cases);
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {test} from 'node:test';
import {measuredRun, sampleResidentBytes, timeResidentBytes} from './native-corpus-resources.mjs';

test('OS resident measurements retain their actual units and reject missing data', () => {
  assert.equal(timeResidentBytes(' 123456 maximum resident set size\n', 'darwin'), 123456);
  assert.equal(timeResidentBytes('\tMaximum resident set size (kbytes): 123456\n', 'linux'), 123456 * 1024);
  for (const output of ['', '0 maximum resident set size\n',
    '1 maximum resident set size\n2 maximum resident set size\n',
    '999999999999999999999999 maximum resident set size\n']) {
    assert.throws(() => timeResidentBytes(output, 'darwin'));
  }
  assert.throws(() => timeResidentBytes('', 'unsupported'));
});

test('measurement preserves literal arguments, application output and failure status', () => {
  const literal = 'value $(literal) with "quotes", spaces and\na newline';
  const run = measuredRun(process.execPath, ['-e',
    'process.stdout.write(process.argv[1]); process.stderr.write("application failure"); process.exitCode = 7;', literal], {
    timeout: 10_000, maxBuffer: 1024 * 1024,
  });
  assert.ifError(run.error);
  assert.equal(run.resourceMeasurementFailure, undefined);
  assert.equal(run.status, 7);
  assert.equal(run.stdout, literal);
  assert.equal(run.stderr, 'application failure');
  assert.ok(run.maximumResidentBytes > 0);
  assert.notEqual(run.resourceMonitorOutput, run.stderr);
});

test('service resident samples validate the actual process ID', () => {
  assert.ok(sampleResidentBytes(process.pid) > 0);
  for (const pid of [0, -1, 1.5, '123', Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(() => sampleResidentBytes(pid));
  }
});

test('a measurement timeout also stops the application beneath its monitor', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-monitor-'));
  const file = path.join(directory, 'pid');
  let pid;
  try {
    const run = measuredRun(process.execPath, ['-e',
      'require("node:fs").writeFileSync(process.argv[1], String(process.pid)); setInterval(() => {}, 1000);', file], {
      timeout: 2000, maxBuffer: 1024 * 1024,
    });
    assert.equal(run.error?.code, 'ETIMEDOUT');
    assert.equal(run.resourceCleanupFailure, undefined);
    pid = Number(fs.readFileSync(file, 'utf8'));
    const deadline = performance.now() + 5000;
    for (;;) {
      try {
        process.kill(pid, 0);
      } catch (error) {
        assert.equal(error.code, 'ESRCH');
        break;
      }
      assert.ok(performance.now() < deadline, 'timed-out application remains alive');
      await new Promise(resolve => setTimeout(resolve, 25));
    }
  } finally {
    if (pid) {
      try { process.kill(pid, 'SIGKILL'); }
      catch (error) { if (error.code !== 'ESRCH') throw error; }
    }
    fs.rmSync(directory, {recursive: true});
  }
});

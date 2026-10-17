// OS measurements stay outside the product protocol and logical budget ledger.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';

function bytes(value, multiplier) {
  const result = Number(value) * multiplier;
  assert.ok(Number.isSafeInteger(result) && result > 0, 'invalid resident memory measurement');
  return result;
}

export function timeResidentBytes(output, platform) {
  const pattern = platform === 'darwin' ? /^\s*(\d+)\s+maximum resident set size\s*$/gm
    : platform === 'linux' ? /^\s*Maximum resident set size \(kbytes\):\s*(\d+)\s*$/gm
      : undefined;
  assert.ok(pattern, `resource measurement is unavailable on ${platform}`);
  const matches = [...output.matchAll(pattern)];
  assert.equal(matches.length, 1, 'missing or ambiguous maximum resident memory measurement');
  return bytes(matches[0][1], platform === 'darwin' ? 1 : 1024);
}

export function measuredRun(command, args, options) {
  assert.ok(['darwin', 'linux'].includes(process.platform), `resource measurement is unavailable on ${process.platform}`);
  // The fixed shell only redirects the child's stderr to a separate pipe.
  // Every application argument remains a quoted positional argument.
  const run = spawnSync('/usr/bin/time', [process.platform === 'darwin' ? '-l' : '-v',
    '/bin/sh', '-c', 'exec "$@" 2>&3', 'resource-child', command, ...args], {
    ...options, env: {...(options.env ?? process.env), LC_ALL: 'C'},
    stdio: ['ignore', 'pipe', 'pipe', 'pipe'], encoding: 'utf8',
    detached: true, killSignal: 'SIGKILL',
  });
  const result = {...run, stderr: run.output?.[3] ?? '', resourceMonitorOutput: run.stderr};
  // A timeout kills the monitor first. Its entire private process group must
  // also stop, including a direct CLI still running beneath that monitor.
  if (run.error?.code === 'ETIMEDOUT' && run.pid > 0) {
    try {
      process.kill(-run.pid, 'SIGKILL');
    } catch (error) {
      if (error.code !== 'ESRCH') result.resourceCleanupFailure = String(error);
    }
  }
  if (!run.error) {
    try {
      result.maximumResidentBytes = timeResidentBytes(run.stderr, process.platform);
    } catch (error) {
      result.resourceMeasurementFailure = String(error);
    }
  }
  return result;
}

// This is a sample after a query, not a claim about the server's peak RSS.
export function sampleResidentBytes(pid) {
  assert.ok(Number.isSafeInteger(pid) && pid > 0, 'invalid service process ID');
  const run = spawnSync('ps', ['-o', 'rss=', '-p', String(pid)], {
    encoding: 'utf8', timeout: 5000, maxBuffer: 64 * 1024,
  });
  assert.ifError(run.error);
  assert.equal(run.status, 0, run.stderr);
  assert.match(run.stdout, /^\s*\d+\s*$/);
  return bytes(run.stdout.trim(), 1024);
}

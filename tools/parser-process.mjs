import { spawnSync } from 'node:child_process';

// The Rust CLI contract is 0 = accepted, 1 = syntax diagnostic, 2 = usage/I/O
// failure. Signals, timeouts and launch errors are never syntax rejections.
// Native compiler processes have a different exit contract; do not use this
// classifier for them.
export function runParser(binary, args, options = {}) {
  const result = spawnSync(binary, args, {
    timeout: 10_000, maxBuffer: 64 * 1024 * 1024, ...options, encoding: 'utf8',
  });
  const outcome = result.error || result.signal ? 'failed'
    : result.status === 0 ? 'accepted' : result.status === 1 ? 'rejected' : 'failed';
  return { ...result, outcome };
}

export function parserFailure(result) {
  return {
    error: result.error?.message || result.stderr?.trim()
      || `Parser process failed (exit ${result.status}, signal ${result.signal})`,
    status: result.status, signal: result.signal,
  };
}

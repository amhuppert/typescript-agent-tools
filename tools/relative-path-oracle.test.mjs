import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {pinnedGoSource} from './type-oracle.mjs';

test('relative paths match pinned TypeScript 7.0.2', () => {
  const root = fileURLToPath(new URL('../', import.meta.url));
  const fixture = path.join(root, 'fixtures/paths/relative.json');
  const output = path.join(root, '.cc/temp/relative-path-native.json');
  fs.mkdirSync(path.dirname(output), {recursive: true});
  fs.rmSync(output, {force: true});
  const source = pinnedGoSource();
  const temporary = fs.mkdtempSync(path.join(root, '.cc/temp/relative-path-overlay-'));
  try {
    const generated = path.join(temporary, 'native.json');
    const overlay = path.join(temporary, 'overlay.json');
    fs.writeFileSync(overlay, JSON.stringify({Replace: {
      [path.join(source, 'internal/tspath/zz_agent_relative_test.go')]:
        path.join(root, 'fixtures/paths/oracle_test.go'),
    }}));
    const result = spawnSync('go', ['test', '-overlay', overlay, './internal/tspath',
      '-run', '^TestAgentRelativePaths$', '-count=1'], {
      cwd: source, encoding: 'utf8', timeout: 180_000, maxBuffer: 1024 * 1024,
      env: {...process.env, AGENT_PATH_CASES: fixture, AGENT_PATH_OUTPUT: generated},
    });
    assert.ifError(result.error);
    assert.equal(result.signal, null, result.stderr);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    // A successful Go command that ran no matching test must not reuse an
    // earlier answer. Only this invocation can create the temporary output.
    const actual = fs.readFileSync(generated, 'utf8');
    fs.writeFileSync(output, actual);
    const expected = JSON.parse(fs.readFileSync(fixture, 'utf8'));
    assert.ok(expected.length > 0);
    assert.deepEqual(JSON.parse(actual), expected);
  } finally {
    fs.rmSync(temporary, {recursive: true, force: true});
  }
});

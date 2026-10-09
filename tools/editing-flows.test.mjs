import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { runFlows, withChanges } from './editing-flows.mjs';

// The editing proof tool on a small project. It drives a built `tsparse`;
// without one these tests are skipped with this reason.
const tsparse = process.env.TSPARSE ?? fileURLToPath(new URL('../target/debug/tsparse', import.meta.url));
const unbuilt = !fs.existsSync(tsparse) && `no tsparse at ${tsparse}; build it or set TSPARSE`;
const unixOnly = process.platform === 'win32' && 'the project service and apply are Unix-only';

const FILES = {
  'tsconfig.json': '{ "compilerOptions": { "noLib": true, "types": [] }, "include": ["src"] }',
  'src/a.ts': '﻿export function run() {\n  return 1;\n}\n\nexport const helper = () => 2;\n',
  'src/b.ts': 'import { run } from "./a";\nrun();\n',
};

const SCENARIO = {
  flows: [
    {
      name: 'body and insertion in two files',
      edits: [
        { operation: 'replace_body', name: 'run', file: 'src/a.ts', fragment: '{\n  return helper();\n}' },
        { operation: 'insert_after', at: 'src/b.ts:1:10', fragment: 'export const value = run();' },
      ],
      then: ['--show --name value src/b.ts', '--references --name run'],
    },
  ],
};

function project(t) {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-edit-flows-')));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const [file, text] of Object.entries(FILES)) {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), text);
  }
  return { root, config: path.join(root, 'tsconfig.json') };
}

test('changes apply at byte offsets of the original text', () => {
  assert.equal(
    withChanges('é = 1;\nb;\n', [{ span: { start: 7, end: 7 }, text: '\nc;' }, { span: { start: 5, end: 6 }, text: '2' }]),
    'é = 2;\nc;\nb;\n',
  );
});

test('built tsparse: editing flows hold on a small project', { skip: unbuilt || unixOnly }, t => {
  const { root, config } = project(t);
  const report = runFlows({ tsparse, config, scenario: SCENARIO });
  assert.deepEqual(report.failures, []);
  assert.deepEqual(report.flows[0].files, ['src/a.ts', 'src/b.ts']);
  assert.deepEqual(report.flows[0].then.map(request => request.status), [0, 0]);
  // The byte order mark stays; the edits are made.
  const a = fs.readFileSync(path.join(root, 'src/a.ts'), 'utf8');
  assert.equal(a, '﻿export function run() {\n  return helper();\n}\n\nexport const helper = () => 2;\n');
  assert.equal(fs.readFileSync(path.join(root, 'src/b.ts'), 'utf8'), 'import { run } from "./a";\nexport const value = run();\nrun();\n');
});

test('built tsparse: an apply that writes other bytes than the preview fails the flows', { skip: unbuilt || unixOnly }, t => {
  const { root, config } = project(t);
  // A stand-in that appends to every file an apply replaced.
  const wrapper = path.join(root, 'tsparse-wrapper');
  fs.writeFileSync(wrapper, `#!/bin/sh\ncase " $* " in *" --apply "*) "${tsparse}" "$@"; status=$?; printf '// extra\\n' >> "${root}/src/b.ts"; exit $status;; esac\nexec "${tsparse}" "$@"\n`, { mode: 0o755 });
  const report = runFlows({ tsparse: wrapper, config, scenario: SCENARIO });
  assert.ok(report.failures.some(failure => failure.what === 'src/b.ts differs from the preview'), JSON.stringify(report.failures));
  const scenario = path.join(root, 'scenario.json');
  fs.writeFileSync(scenario, JSON.stringify(SCENARIO));
  fs.writeFileSync(path.join(root, 'src/a.ts'), FILES['src/a.ts']);
  fs.writeFileSync(path.join(root, 'src/b.ts'), FILES['src/b.ts']);
  const result = spawnSync(process.execPath, [fileURLToPath(new URL('./editing-flows.mjs', import.meta.url)), config, scenario,
    `--report=${path.join(root, 'report.json')}`], { cwd: root, env: { ...process.env, TSPARSE: wrapper }, encoding: 'utf8' });
  assert.equal(result.status, 1, result.stdout + result.stderr);
});

test('built tsparse: native checks run the compiler and fail when it cannot run', { skip: unbuilt || unixOnly }, t => {
  const { root, config } = project(t);
  // A stand-in compiler that reports a new diagnostic once a file is edited.
  const native = path.join(root, 'tsc-shim');
  fs.writeFileSync(native, `#!/bin/sh\nfor f; do case "$f" in *.ts) if grep -q helper "$f" 2>/dev/null && grep -q "return helper" "$f"; then echo "$f(1,1): error TS9999: edited"; exit 2; fi;; esac; done\nexit 0\n`, { mode: 0o755 });
  const report = runFlows({ tsparse, config, scenario: SCENARIO, native });
  assert.deepEqual(report.failures.map(failure => failure.what), ['src/a.ts: the native compiler reports new diagnostics']);
  assert.equal(report.summary.native, true);
  // A compiler that cannot run fails the flow instead of passing silently.
  fs.writeFileSync(path.join(root, 'src/a.ts'), FILES['src/a.ts']);
  fs.writeFileSync(path.join(root, 'src/b.ts'), FILES['src/b.ts']);
  const missing = runFlows({ tsparse, config, scenario: SCENARIO, native: path.join(root, 'no-such-tsc') });
  assert.ok(missing.failures.some(failure => failure.what.startsWith('the native compiler did not run')), JSON.stringify(missing.failures));
});

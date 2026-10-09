// Editing flows on a real project through the CLI and its service: the
// reproducible form of the editing proof on a disposable project copy
// (docs/validation.md, "Editing on Command Center"). It needs a built
// `tsparse` and a copy of the project it may change; the native compiler
// is optional.
//
//   TSPARSE=/abs/target/release/tsparse [TYPE_SCRIPT_NATIVE=/abs/tsc] \
//     node tools/editing-flows.mjs /abs/copy/tsconfig.json SCENARIO.json
//       [--report=PATH]
//
// SCENARIO.json is `{ "flows": [{ "name", "edits", "then" }] }`: `edits` is an
// `--edit-batch` list whose paths are relative to the configuration's
// directory, and `then` lists requests, as arguments split at spaces and
// given without `--project`, to answer after applying. For each flow, in
// order:
//   - the batch's JSON preview through the service, saving a plan, must
//     succeed, and `--direct` must give the same document but its `plan`;
//     the text preview must succeed too;
//   - applying the plan must replace exactly the previewed files, and each
//     file must then hold its bytes before the flow with the preview's
//     changes made, after any byte order mark;
//   - with TYPE_SCRIPT_NATIVE, the native compiler executable's `--noCheck`
//     run over each changed file must complete and report no diagnostic code
//     it did not report before; a compiler that does not run fails the flow;
//   - each `then` request must succeed through the service; the first one
//     answers from a refreshed project;
//   - applying the plan again must fail as stale and change nothing.
// The report (default `.cache/editing-flows.json`) records each flow's
// timings, files, and failures. The runner exits 1 on any failure and 2
// when it cannot run.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

const BOM = Buffer.from([0xef, 0xbb, 0xbf]);

// The analyzed text of a file's bytes, and the byte order mark before it.
function decode(bytes) {
  const marked = bytes.subarray(0, 3).equals(BOM);
  return { marked, text: (marked ? bytes.subarray(3) : bytes).toString('utf8') };
}

// `text` with the preview's changes made: byte spans of the original text.
export function withChanges(text, changes) {
  const bytes = Buffer.from(text, 'utf8');
  const ordered = [...changes].sort((a, b) => a.span.start - b.span.start);
  const parts = [];
  let at = 0;
  for (const change of ordered) {
    parts.push(bytes.subarray(at, change.span.start), Buffer.from(change.text, 'utf8'));
    at = change.span.end;
  }
  parts.push(bytes.subarray(at));
  return Buffer.concat(parts).toString('utf8');
}

// The diagnostic codes the native compiler, an executable, reports for
// `file` without checking, as a set; or an Error when it did not run to
// completion: a spawn failure, a signal, or an exit other than 0 without
// diagnostics, 1, or 2 with them.
function nativeCodes(native, file) {
  const result = spawnSync(native, ['--noEmit', '--noCheck', '--ignoreConfig',
    '--target', 'esnext', '--module', 'esnext', '--jsx', 'preserve', '--pretty', 'false', file],
  { encoding: 'utf8', maxBuffer: 1 << 28 });
  const output = `${result.stdout ?? ''}`;
  const codes = new Set([...output.matchAll(/error (TS\d+)/g)].map(match => match[1]));
  if (result.error) return new Error(result.error.message);
  if (result.signal) return new Error(`killed by ${result.signal}`);
  if (result.status === 0 ? codes.size : !(result.status === 1 || result.status === 2) || !codes.size)
    return new Error(`exit ${result.status}: ${(result.stderr || output).slice(0, 300)}`);
  return codes;
}

export function runFlows({ tsparse, config, scenario, native = null }) {
  const root = path.dirname(config);
  const runtime = fs.mkdtempSync(path.join('/tmp', 'tsp-edit-'));
  fs.chmodSync(runtime, 0o700);
  const env = { ...process.env, TSPARSE_SERVICE_DIR: runtime };
  const failures = [];
  const records = [];
  const exec = (argv, { direct = false } = {}) => {
    const started = process.hrtime.bigint();
    const result = spawnSync(tsparse, direct ? [...argv, '--direct'] : argv,
      { cwd: root, env, encoding: 'utf8', maxBuffer: 1 << 30 });
    const ms = Number(process.hrtime.bigint() - started) / 1e6;
    if (result.error) throw result.error;
    return { status: result.status, stdout: result.stdout, stderr: result.stderr, ms: Math.round(ms) };
  };
  const project = ['--project', config];
  try {
    scenario.flows.forEach((flow, index) => {
      const fail = (what, detail = null) => failures.push({ flow: flow.name, what, detail });
      const record = { name: flow.name, edits: flow.edits.length };
      records.push(record);
      const batch = path.join(runtime, `edits-${index}.json`);
      const plan = path.join(runtime, `plan-${index}.json`);
      fs.writeFileSync(batch, JSON.stringify({ edits: flow.edits }));
      const preview = exec(['--edit-batch', batch, ...project, '--json', '--plan', plan]);
      record.previewMs = preview.ms;
      let document;
      try {
        document = JSON.parse(preview.stdout);
      } catch {
        fail('preview is not JSON', preview.stderr.slice(0, 500));
        return;
      }
      if (preview.status !== 0 || document.outcome !== 'preview') {
        fail('preview failed', document.error ?? document.outcome);
        return;
      }
      const direct = exec(['--edit-batch', batch, ...project, '--json'], { direct: true });
      record.directPreviewMs = direct.ms;
      const directDocument = direct.status === 0 ? JSON.parse(direct.stdout) : null;
      if (!directDocument || JSON.stringify({ ...directDocument, plan: null }) !== JSON.stringify({ ...document, plan: null }))
        fail('--direct preview differs from the service preview', direct.stderr.slice(0, 300));
      const text = exec(['--edit-batch', batch, ...project]);
      if (text.status !== 0 || !text.stdout.includes('validated: syntax and edit boundaries'))
        fail('text preview failed', text.stderr.slice(0, 300));
      const changed = document.files.filter(file => file.changed);
      record.files = changed.map(file => file.file);
      record.lines = changed.map(file => file.diff.split('\n').filter(line => /^[+-][^+-]/.test(line)).length);
      const before = new Map(changed.map(file => [file.file, fs.readFileSync(path.join(root, file.file))]));
      const codes = new Map(native ? changed.map(file => [file.file, nativeCodes(native, path.join(root, file.file))]) : []);
      for (const [file, result] of codes)
        if (result instanceof Error) fail(`the native compiler did not run on ${file} before the edit`, result.message);
      const applied = exec(['--apply', plan, '--json']);
      record.applyMs = applied.ms;
      const outcome = applied.status === 0 ? JSON.parse(applied.stdout) : null;
      if (!outcome || outcome.effect !== 'applied') {
        fail('apply failed', applied.stdout.slice(0, 500) || applied.stderr.slice(0, 500));
        return;
      }
      const replaced = outcome.files.filter(file => file.state === 'replaced').map(file => file.display);
      if (JSON.stringify([...replaced].sort()) !== JSON.stringify([...record.files].sort()))
        fail('apply replaced other files than the preview changed', { replaced, previewed: record.files });
      for (const file of changed) {
        const original = decode(before.get(file.file));
        const expected = withChanges(original.text, file.edits.flatMap(edit => edit.changes));
        const now = decode(fs.readFileSync(path.join(root, file.file)));
        if (now.marked !== original.marked || now.text !== expected) fail(`${file.file} differs from the preview`);
        const baseline = codes.get(file.file);
        if (native && !(baseline instanceof Error)) {
          const after = nativeCodes(native, path.join(root, file.file));
          if (after instanceof Error) {
            fail(`the native compiler did not run on ${file.file} after the edit`, after.message);
          } else {
            const added = [...after].filter(code => !baseline.has(code));
            if (added.length) fail(`${file.file}: the native compiler reports new diagnostics`, added);
          }
        }
      }
      record.then = [];
      for (const request of flow.then ?? []) {
        const answered = exec([...request.split(' ').filter(Boolean), ...project]);
        record.then.push({ request, status: answered.status, ms: answered.ms });
        if (answered.status !== 0) fail(`then: ${request}`, answered.stderr.slice(0, 300) || answered.stdout.slice(0, 300));
      }
      const again = exec(['--apply', plan, '--json']);
      const stale = again.status === 1 && JSON.parse(again.stdout).effect === 'unchanged';
      if (!stale) fail('applying the plan again did not fail as stale', again.stdout.slice(0, 300));
    });
  } finally {
    exec(['--references-stop', ...project]);
    fs.rmSync(runtime, { recursive: true, force: true });
  }
  const summary = {
    flows: records.length, files: records.reduce((sum, record) => sum + (record.files?.length ?? 0), 0),
    native: Boolean(native), failures: failures.length,
  };
  return { config, tsparse, summary, flows: records, failures };
}

function parseArguments(args) {
  const options = { report: '.cache/editing-flows.json' };
  for (const arg of args) {
    if (arg.startsWith('--report=')) options.report = arg.slice('--report='.length);
    else if (arg.startsWith('-')) throw new Error(`Unknown option ${arg}`);
    else if (options.config === undefined) options.config = arg;
    else if (options.scenario === undefined) options.scenario = arg;
    else throw new Error(`Unexpected argument ${arg}`);
  }
  if (options.config === undefined || options.scenario === undefined)
    throw new Error('Usage: node tools/editing-flows.mjs TSCONFIG SCENARIO.json [--report=PATH]');
  return options;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  try {
    const options = parseArguments(process.argv.slice(2));
    const tsparse = process.env.TSPARSE ?? path.resolve('target/release/tsparse');
    if (!fs.existsSync(tsparse)) throw new Error(`no tsparse at ${tsparse}; build it or set TSPARSE`);
    const scenario = JSON.parse(fs.readFileSync(options.scenario, 'utf8'));
    const report = runFlows({
      tsparse, scenario, native: process.env.TYPE_SCRIPT_NATIVE ?? null,
      config: fs.realpathSync(path.resolve(options.config)),
    });
    fs.mkdirSync(path.dirname(path.resolve(options.report)), { recursive: true });
    fs.writeFileSync(options.report, `${JSON.stringify(report, null, 1)}\n`);
    for (const flow of report.flows) console.log(JSON.stringify(flow));
    console.log(JSON.stringify(report.summary));
    for (const failure of report.failures.slice(0, 30)) console.log('FAIL', JSON.stringify(failure));
    process.exitCode = report.failures.length ? 1 : 0;
  } catch (error) {
    console.error(`editing-flows: ${error.message}`);
    process.exitCode = 2;
  }
}

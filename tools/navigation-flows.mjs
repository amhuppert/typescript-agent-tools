// Navigation flows on a real project through the CLI and its service: the
// reproducible form of the Command Center proof (docs/validation.md,
// "Command Center checkpoint"). It needs a built `tsparse` and the project,
// not the compilers.
//
//   TSPARSE=/abs/target/release/tsparse \
//     node tools/navigation-flows.mjs /abs/app/tsconfig.json [--report=PATH]
//       [--direct-sample=N] [--request=ARGS ...] [FILE ...]
//
// Each FILE is a searched file, relative to the configuration's directory.
// For each one:
//   - its project outline (`--json --depth all`, through the service) has
//     the entries of its file-mode outline;
//   - for sampled entries (the first, the last, the deepest, those without
//     a selector up to 14 picks, then a spread up to 24), `--show --at` the
//     entry's anchor finds it with the outline's anchor, range, kind, and
//     label; its `source` is the file's text over that range, as TypeScript
//     reads the file; and the text form prints the same lines numbered as
//     in the file;
//   - for a sampled entry with a selector, a project-wide `--show --name`
//     finds the entry, or is ambiguous with it among the candidates. Then up
//     to four JSON candidates, their file joined to the configuration's
//     directory, select their declaration again; the text form must list
//     the first two as `FILE:LINE:COLUMN kind label (selector)` lines, and
//     each of those locations, whole, selects its declaration with `--at`.
// `--direct-sample=N` repeats the first N `--show --at` requests with
// `--direct`, which must give the service's exit status and bytes.
// `--request=ARGS` adds one request, its arguments split at spaces and
// given without `--project`, answered through the service and with
// `--direct`; both must give the same exit status and bytes, and the report
// records each one's exit status, output size, times, and answer: a
// reference result as its groups' declaration and occurrence counts.
//
// The service runs in a private directory under /tmp, created for the run,
// and is stopped at the end. The report (default
// `.cache/navigation-flows.json`) holds a record per file and request, a
// summary, and every failure. The runner exits 1 on any failure and 2 when
// it cannot run.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { lineStarts, sourceText } from './navigation-oracle.mjs';

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

// Outline entries in pre-order, with depth.
function flatten(entries, depth = 1, out = []) {
  for (const entry of entries) {
    out.push({ ...entry, depth });
    flatten(entry.children ?? [], depth + 1, out);
  }
  return out;
}

// The text TypeScript reads for `file`, and UTF-16 offsets of positions.
function captured(file) {
  const text = sourceText(file);
  const starts = lineStarts(text);
  return { text, offset: position => starts[position.line - 1] + position.column - 1 };
}

// The entries to sample: first, last, deepest, those without a selector,
// then a spread.
export function samples(entries) {
  if (!entries.length) return [];
  const picks = new Set([0, entries.length - 1]);
  let deepest = 0;
  entries.forEach((entry, index) => { if (entry.depth > entries[deepest].depth) deepest = index; });
  picks.add(deepest);
  entries.forEach((entry, index) => { if (!entry.selector && picks.size < 14) picks.add(index); });
  const step = Math.max(1, Math.floor(entries.length / 10));
  for (let index = 0; index < entries.length && picks.size < 24; index += step) picks.add(index);
  return [...picks].sort((a, b) => a - b);
}

// Run the flows; returns the report.
export function runFlows({ tsparse, config, files = [], requests = [], directSample = 0 }) {
  const root = path.dirname(config);
  const runtime = fs.mkdtempSync(path.join('/tmp', 'tsp-flows-'));
  fs.chmodSync(runtime, 0o700);
  const env = { ...process.env, TSPARSE_SERVICE_DIR: runtime };
  const failures = [];
  const fail = (scope, what, detail = null) => failures.push({ scope, what, detail });
  const counts = { serviceRequests: 0, serviceMs: 0, directCompared: 0 };
  const exec = (argv, { direct = false, cwd = root } = {}) => {
    const started = process.hrtime.bigint();
    const result = spawnSync(tsparse, direct ? [...argv, '--direct'] : argv,
      { cwd, env, encoding: 'utf8', maxBuffer: 1 << 30 });
    const ms = Number(process.hrtime.bigint() - started) / 1e6;
    if (result.error) throw result.error;
    return { status: result.status, stdout: result.stdout, stderr: result.stderr, ms };
  };
  const service = argv => {
    const result = exec(argv);
    counts.serviceRequests++;
    counts.serviceMs += result.ms;
    return result;
  };
  const project = ['--project', config];
  const identical = (scope, argv, served) => {
    const direct = exec(argv, { direct: true });
    counts.directCompared++;
    if (direct.status !== served.status || direct.stdout !== served.stdout || direct.stderr !== served.stderr)
      fail(scope, `--direct differs: ${argv.join(' ')}`, { service: served.status, direct: direct.status });
    return direct;
  };
  const records = [];
  const requestRecords = [];
  let directBudget = directSample;
  try {
    for (const file of files) {
      const outlined = service(['--outline', ...project, '--json', '--depth', 'all', file]);
      if (outlined.status !== 0) { fail(file, 'outline failed', outlined.stdout.slice(0, 300) || outlined.stderr); continue; }
      const outline = JSON.parse(outlined.stdout);
      const fileMode = exec(['--outline', '--json', '--depth', 'all', file]);
      if (fileMode.status !== 0 || !same(outline.entries, JSON.parse(fileMode.stdout).entries))
        fail(file, 'project outline differs from the file outline');
      const entries = flatten(outline.entries);
      const source = captured(path.join(root, file));
      const record = {
        file, entries: entries.length, maxDepth: Math.max(0, ...entries.map(entry => entry.depth)),
        outlineMs: Math.round(outlined.ms), shows: 0, names: 0, found: 0, ambiguous: 0, roundTrips: 0, positionOnly: 0,
      };
      for (const index of samples(entries)) {
        const entry = entries[index];
        if (!entry.selector) record.positionOnly++;
        const at = `${entry.anchor.line}:${entry.anchor.column}`;
        const argv = ['--show', ...project, '--at', at, file, '--json'];
        const shown = service(argv);
        record.shows++;
        if (directBudget > 0) {
          directBudget--;
          identical(file, argv, shown);
        }
        const document = JSON.parse(shown.stdout);
        if (shown.status !== 0 || document.outcome !== 'found') { fail(file, `--show --at ${at} not found`, shown.stdout.slice(0, 300)); continue; }
        const declaration = document.declaration;
        if (document.file !== file) fail(file, `--show --at ${at} names ${document.file}`);
        for (const key of ['anchor', 'range', 'kind', 'label'])
          if (!same(declaration[key], entry[key])) fail(file, `--show --at ${at} ${key}`, { show: declaration[key], outline: entry[key] });
        const slice = source.text.slice(source.offset(declaration.range.start), source.offset(declaration.range.end));
        if (slice !== document.source) fail(file, `excerpt at ${at}`, { expected: slice.slice(0, 80), got: document.source.slice(0, 80) });
        // The text form: the same lines, numbered as in the file.
        const text = service(['--show', ...project, '--at', at, file]);
        const lines = text.stdout.split('\n');
        const header = `${path.join(root, file)}:${at} ${declaration.kind} ${declaration.label}`;
        if (text.status !== 0 || lines[0] !== header) fail(file, `text show header at ${at}`, lines[0]);
        const body = lines.slice(1, -1).map(line => line.replace(/^ *(\d+) \|(?: |$)/, '$1|'));
        const expected = document.source.split(/\r\n|\r|\n|\u2028|\u2029/)
          .map((line, offset) => `${declaration.range.start.line + offset}|${line}`);
        if (!same(body, expected)) fail(file, `text show body at ${at}`, { got: body.slice(0, 3), expected: expected.slice(0, 3) });
        if (!entry.selector) continue;
        // The entry's qualified name across the project, and its candidates.
        const named = service(['--show', ...project, '--name', entry.selector, '--json']);
        record.names++;
        const result = JSON.parse(named.stdout);
        if (result.outcome === 'found') {
          record.found++;
          if (named.status !== 0 || result.file !== file || !same(result.declaration.anchor, entry.anchor))
            fail(file, `--name ${entry.selector} found another declaration`, { file: result.file, anchor: result.declaration.anchor });
          continue;
        }
        if (result.outcome !== 'ambiguous') { fail(file, `--name ${entry.selector} ${result.outcome}`); continue; }
        record.ambiguous++;
        if (named.status !== 1 || named.stderr) fail(file, `--name ${entry.selector} ambiguous channels`);
        if (!result.candidates.some(candidate => candidate.file === file && same(candidate.anchor, entry.anchor)))
          fail(file, `--name ${entry.selector} lacks the entry`);
        const base = path.dirname(result.project);
        for (const candidate of result.candidates.slice(0, 4)) {
          const target = path.join(base, candidate.file);
          const again = service(['--show', ...project, '--at', `${candidate.anchor.line}:${candidate.anchor.column}`, target, '--json']);
          record.roundTrips++;
          const found = JSON.parse(again.stdout);
          if (again.status !== 0 || found.outcome !== 'found' || found.file !== candidate.file
            || !same(found.declaration.anchor, candidate.anchor) || found.declaration.selector !== candidate.selector)
            fail(file, `round trip ${candidate.file}:${candidate.anchor.line}:${candidate.anchor.column}`, again.stdout.slice(0, 200));
        }
        // The text form lists the same candidates on stderr, each line
        // `FILE:LINE:COLUMN kind label (selector)`. Its locations are taken
        // from the JSON candidates, not parsed out of the text, so a file
        // name with spaces or colons stays whole; the text must contain each
        // one, and each, given whole to `--at`, selects its declaration.
        const textual = service(['--show', ...project, '--name', entry.selector]);
        const listed = textual.stderr.split('\n');
        const printed = result.candidates.slice(0, 2).map(candidate => ({
          place: `${path.join(base, candidate.file)}:${candidate.anchor.line}:${candidate.anchor.column}`,
          line: `${path.join(base, candidate.file)}:${candidate.anchor.line}:${candidate.anchor.column} ${candidate.kind} ${candidate.label}`
            + (candidate.selector ? ` (${candidate.selector})` : ''),
        }));
        const missing = printed.filter(({ line }) => !listed.includes(line));
        if (textual.status !== 1 || textual.stdout || missing.length)
          fail(file, `text candidates for --name ${entry.selector}`, { status: textual.status, missing: missing.map(({ line }) => line) });
        for (const { place } of printed) {
          const again = service(['--show', ...project, '--at', place]);
          record.roundTrips++;
          if (again.status !== 0 || !again.stdout.startsWith(`${place} `)) fail(file, `text round trip ${place}`, again.stderr.slice(0, 200));
        }
      }
      records.push(record);
    }
    for (const request of requests) {
      const argv = [...request.split(' ').filter(Boolean), ...project];
      const served = service(argv);
      const direct = identical(request, argv, served);
      requestRecords.push({
        request, status: served.status, stdoutBytes: Buffer.byteLength(served.stdout),
        stderr: served.stderr.slice(0, 300), serviceMs: Math.round(served.ms), directMs: Math.round(direct.ms),
        ...answer(served.stdout),
      });
    }
  } finally {
    exec(['--references-stop', ...project]);
    fs.rmSync(runtime, { recursive: true, force: true });
  }
  const total = key => records.reduce((sum, record) => sum + record[key], 0);
  const summary = {
    files: records.length, entries: total('entries'), shows: total('shows'), names: total('names'),
    found: total('found'), ambiguous: total('ambiguous'), roundTrips: total('roundTrips'),
    positionOnly: total('positionOnly'), requests: requestRecords.length, serviceRequests: counts.serviceRequests,
    meanServiceMs: counts.serviceRequests ? Math.round(counts.serviceMs / counts.serviceRequests) : 0,
    directCompared: counts.directCompared, failures: failures.length,
  };
  return { config, tsparse, summary, files: records, requests: requestRecords, failures };
}

// What a request's stdout answered: a reference result's groups as their
// declaration and occurrence counts, a document's outcome, and the text,
// abbreviated.
function answer(stdout) {
  const text = stdout.length <= 2000 ? stdout : `${stdout.slice(0, 2000)}…`;
  let document;
  try {
    document = JSON.parse(stdout);
  } catch {
    return { stdout: text };
  }
  if (Array.isArray(document.groups)) {
    return { groups: document.groups.map(group => [group.declarations.length, group.occurrences.length]) };
  }
  return { outcome: document.outcome, stdout: text };
}

function parseArguments(args) {
  const options = { files: [], requests: [], directSample: 0, report: '.cache/navigation-flows.json' };
  for (const arg of args) {
    if (arg.startsWith('--request=')) options.requests.push(arg.slice('--request='.length));
    else if (arg.startsWith('--report=')) options.report = arg.slice('--report='.length);
    else if (arg.startsWith('--direct-sample=')) {
      options.directSample = Number(arg.slice('--direct-sample='.length));
      if (!Number.isSafeInteger(options.directSample) || options.directSample < 0)
        throw new Error('--direct-sample must be a non-negative integer');
    } else if (arg.startsWith('-')) throw new Error(`Unknown option ${arg}`);
    else if (options.config === undefined) options.config = arg;
    else options.files.push(arg);
  }
  if (options.config === undefined || !options.files.length && !options.requests.length) {
    throw new Error('Usage: node tools/navigation-flows.mjs TSCONFIG [--report=PATH] [--direct-sample=N] [--request=ARGS ...] [FILE ...]');
  }
  return options;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  try {
    const options = parseArguments(process.argv.slice(2));
    const tsparse = process.env.TSPARSE ?? path.resolve('target/release/tsparse');
    if (!fs.existsSync(tsparse)) throw new Error(`no tsparse at ${tsparse}; build it or set TSPARSE`);
    const report = runFlows({ ...options, tsparse, config: fs.realpathSync(path.resolve(options.config)) });
    fs.mkdirSync(path.dirname(path.resolve(options.report)), { recursive: true });
    fs.writeFileSync(options.report, `${JSON.stringify(report, null, 1)}\n`);
    for (const record of report.requests) console.log(JSON.stringify({ request: record.request, status: record.status, stdoutBytes: record.stdoutBytes, serviceMs: record.serviceMs, directMs: record.directMs }));
    console.log(JSON.stringify(report.summary));
    for (const failure of report.failures.slice(0, 30)) console.log('FAIL', JSON.stringify(failure));
    process.exitCode = report.failures.length ? 1 : 0;
  } catch (error) {
    console.error(`navigation-flows: ${error.message}`);
    process.exitCode = 2;
  }
}

// How many of a project's exported declarations a project-wide
// `--show --name` selects by their name alone (docs/validation.md,
// "Command Center checkpoint"). It needs a built `tsparse` and the project,
// not the compilers.
//
//   TSPARSE=/abs/target/release/tsparse \
//     node tools/name-selection.mjs /abs/app/tsconfig.json
//       (--listing=LISTING.json | --files=LIST) [--sample=N] [--report=PATH]
//
// The files are the searched TypeScript files: the `project` inputs, other
// than JSON modules, of a `list_configured_program` listing (see
// tools/program-inputs.mjs), or LIST, one path per line, relative to the
// configuration's directory. Each file's file-mode outline (`--outline
// --json --depth all`) gives its entries. A name matches every entry whose
// key's last segment it is (a namespace's key is its dotted chain). The
// measured declarations are the top-level entries with a key, other than
// imports and exports, whose signature begins with `export`, and whose name
// no other entry that is not an import or export has. A project-wide
// `--show --name` finds one exactly when no import or export entry has its
// name either; otherwise it is ambiguous.
//
// `--sample=N` asks the service `--show --project TSCONFIG --json --name` for
// N measured names, evenly spaced, and fails where the answer contradicts
// the prediction; the files must then be the searched files. The report
// (default `.cache/name-selection.json`) holds the counts, the ambiguous
// names, and the sampled answers. The runner exits 1 on a contradiction and
// 2 when it cannot run.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { runnerArguments } from './corpus-selection.mjs';

function* flatten(entries, depth = 1) {
  for (const entry of entries) {
    yield [entry, depth];
    yield* flatten(entry.children ?? [], depth + 1);
  }
}

// The name an unqualified selector spells to match `entry`.
const lastSegment = entry => entry.kind === 'namespace' ? entry.key.split('.').at(-1) : entry.key;
const alias = entry => entry.kind === 'import' || entry.kind === 'export';

// Count names over the outlines' entries.
export function census(outlines) {
  const all = new Map(), declared = new Map();
  const add = (map, name) => map.set(name, (map.get(name) ?? 0) + 1);
  const exported = [];
  for (const { file, entries } of outlines) {
    for (const [entry, depth] of flatten(entries)) {
      if (entry.key === undefined) continue;
      const name = lastSegment(entry);
      add(all, name);
      if (alias(entry)) continue;
      add(declared, name);
      if (depth === 1 && (entry.signature ?? '').startsWith('export')) exported.push({ name, file, anchor: entry.anchor });
    }
  }
  const measured = exported.filter(declaration => declared.get(declaration.name) === 1);
  const ambiguous = measured.filter(declaration => all.get(declaration.name) > 1);
  return { measured, ambiguous, found: measured.length - ambiguous.length };
}

export function runSelection({ tsparse, config, files, sample = 0 }) {
  const root = path.dirname(config);
  const outlines = [];
  const failures = [];
  for (const file of files) {
    const result = spawnSync(tsparse, ['--outline', '--json', '--depth', 'all', file], { cwd: root, encoding: 'utf8', maxBuffer: 1 << 30 });
    if (result.error) throw result.error;
    const document = JSON.parse(result.stdout);
    if (document.outcome !== 'ok') { failures.push({ file, what: 'outline failed', detail: document.error }); continue; }
    outlines.push({ file, entries: document.entries });
  }
  const { measured, ambiguous, found } = census(outlines);
  const ambiguousNames = new Set(ambiguous.map(declaration => declaration.name));
  const sampled = [];
  if (sample > 0 && measured.length) {
    const runtime = fs.mkdtempSync(path.join('/tmp', 'tsp-names-'));
    fs.chmodSync(runtime, 0o700);
    const env = { ...process.env, TSPARSE_SERVICE_DIR: runtime };
    try {
      const step = Math.max(1, Math.floor(measured.length / sample));
      for (let index = 0; index < measured.length && sampled.length < sample; index += step) {
        const { name, file, anchor } = measured[index];
        const started = process.hrtime.bigint();
        const result = spawnSync(tsparse, ['--show', '--project', config, '--json', '--name', name], { cwd: root, env, encoding: 'utf8', maxBuffer: 1 << 30 });
        const ms = Math.round(Number(process.hrtime.bigint() - started) / 1e6);
        if (result.error) throw result.error;
        const document = JSON.parse(result.stdout);
        const predicted = ambiguousNames.has(name) ? 'ambiguous' : 'found';
        sampled.push({ name, predicted, outcome: document.outcome, ms });
        const agrees = predicted === 'found'
          ? document.outcome === 'found' && document.file === file && JSON.stringify(document.declaration.anchor) === JSON.stringify(anchor)
          : document.outcome === 'ambiguous';
        if (!agrees) failures.push({ file, what: `--name ${name}: ${document.outcome}, predicted ${predicted}` });
      }
    } finally {
      spawnSync(tsparse, ['--references-stop', '--project', config], { cwd: root, env });
      fs.rmSync(runtime, { recursive: true, force: true });
    }
  }
  return {
    config, tsparse, files: files.length,
    summary: {
      measured: measured.length, found, ambiguous: ambiguous.length, sampled: sampled.length,
      sampledFound: sampled.filter(answer => answer.outcome === 'found').length, failures: failures.length,
    },
    ambiguous: ambiguous.map(declaration => `${declaration.file}:${declaration.anchor.line}:${declaration.anchor.column} ${declaration.name}`),
    sampled, failures,
  };
}

// The searched TypeScript files of a `list_configured_program` listing.
export function listedFiles(listing) {
  return listing.files.filter(file => file.origin === 'project' && !file.file.endsWith('.json')).map(file => file.file);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  try {
    const { values, paths } = runnerArguments(process.argv.slice(2), {
      options: ['--listing=', '--files=', '--sample=', '--report='], positional: 1,
    });
    const usage = 'Usage: node tools/name-selection.mjs TSCONFIG (--listing=LISTING.json | --files=LIST) [--sample=N] [--report=PATH]';
    if (paths.length !== 1 || values.has('--listing=') === values.has('--files=')) throw new Error(usage);
    const config = fs.realpathSync(path.resolve(paths[0]));
    const files = values.has('--listing=')
      ? listedFiles(JSON.parse(fs.readFileSync(values.get('--listing='), 'utf8')))
      : fs.readFileSync(values.get('--files='), 'utf8').split('\n').filter(Boolean);
    const sample = Number(values.get('--sample=') ?? 0);
    if (!Number.isSafeInteger(sample) || sample < 0) throw new Error('--sample must be a non-negative integer');
    const tsparse = process.env.TSPARSE ?? path.resolve('target/release/tsparse');
    if (!fs.existsSync(tsparse)) throw new Error(`no tsparse at ${tsparse}; build it or set TSPARSE`);
    const report = runSelection({ tsparse, config, files, sample });
    const output = values.get('--report=') ?? '.cache/name-selection.json';
    fs.mkdirSync(path.dirname(path.resolve(output)), { recursive: true });
    fs.writeFileSync(output, `${JSON.stringify(report, null, 1)}\n`);
    console.log(JSON.stringify({ files: report.files, ...report.summary }));
    for (const failure of report.failures.slice(0, 30)) console.log('FAIL', JSON.stringify(failure));
    process.exitCode = report.failures.length ? 1 : 0;
  } catch (error) {
    console.error(`name-selection: ${error.message}`);
    process.exitCode = 2;
  }
}

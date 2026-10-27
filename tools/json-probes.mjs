// JSON-module and configuration probes: the pinned TypeScript 7.0.2 answers
// that the Rust tests of JSON inputs, configuration conversion, and resolution
// admission take their expectations from.
//
//   TYPE_SCRIPT_SEMANTIC_API=/path/to/typescript-7.0.2/package \
//   TYPE_SCRIPT_NATIVE=/path/to/typescript-<platform>-7.0.2/package/lib/tsc \
//     node tools/json-probes.mjs [--update] [--only=NAME,...] [FILE ...]
//
// Each FILE (by default every fixtures/json-probes/*.json) holds small
// projects: `cases`, in order, each with a `name`, its `files` (a case without
// `tsconfig.json` uses the file's `config`), and optionally `watch`, the
// identifier spellings to classify in `watchFile` (`main.ts`, else
// `main.d.ts`), and `showConfig`, the compiler options whose values
// `tsc --showConfig` reports. A case records the compiler's answer in
// `native`:
//
//   program      Program files in `GetSourceFiles` order, relative to the
//                case, a standard library as `lib:<name>`, and a file from an
//                external library marked ` (external)`.
//   diagnostics  `TS<code> <file>:<line>:<column>` for configuration,
//                program, syntactic, and semantic diagnostics, in that order,
//                as `tsc` reports them: binding diagnostics are among the
//                semantic ones of the files TypeScript checks (one-based
//                lines and UTF-16 columns; `-` for a diagnostic without a
//                file). TS2318, a missing global type under `noLib`, is
//                omitted.
//   names        `<line>:<column> <name> -> <target>` for each watched
//                identifier: `none` for no symbol or the unknown symbol,
//                otherwise the aliased symbol's flags and declarations.
//   config       The `tsc --showConfig` values of the `showConfig` keys
//                (`files`, `include`, and `exclude` are top-level keys; others
//                are compiler options), or its exit status when it fails.
//
// Cases may also carry `tool`, the Rust tool's expected outcome, which the
// Rust tests read; this runner never changes it. A case's answers hold on a
// case-insensitive file system; one whose answers differ where file names keep
// their case adds `caseSensitive`, a `native` and `tool` that replace them
// there. The compiler's own case policy (`toPath`) selects the `native` this
// runner compares and updates. Without --update the runner
// compares each recorded `native` with a fresh answer and exits 1 on any
// difference; --update rewrites `native` instead. --only selects cases by
// name.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { isDeepStrictEqual } from 'node:util';
import { fileURLToPath } from 'node:url';
import { loadSemanticApi, lineStarts, position } from './semantic-oracle.mjs';
import { sourceText } from './navigation-oracle.mjs';

export const FIXTURES = fileURLToPath(new URL('../fixtures/json-probes/', import.meta.url));

// The fixture files under `directory`, sorted.
export function fixtureFiles(directory = FIXTURES) {
  return fs.readdirSync(directory).filter(name => name.endsWith('.json')).sort().map(name => path.join(directory, name));
}

// Parse and check one fixture file's shape: unique case names, files, and
// recorded answers.
export function readFixture(file) {
  const fixture = JSON.parse(fs.readFileSync(file, 'utf8'));
  if (typeof fixture.description !== 'string') throw new Error(`${file}: missing description`);
  if (!Array.isArray(fixture.cases) || fixture.cases.length === 0) throw new Error(`${file}: no cases`);
  const names = new Set();
  for (const spec of fixture.cases) {
    if (typeof spec.name !== 'string' || names.has(spec.name)) throw new Error(`${file}: missing or repeated case name ${spec.name}`);
    names.add(spec.name);
    if (!spec.files || typeof spec.files !== 'object' || Object.keys(spec.files).length === 0) throw new Error(`${file}: ${spec.name} has no files`);
    if (spec.files['tsconfig.json'] === undefined && typeof fixture.config !== 'string') throw new Error(`${file}: ${spec.name} has no configuration`);
  }
  return fixture;
}

const display = (root, fileName) => path.relative(root, fs.realpathSync(fileName)).split(path.sep).join('/');

// One case's native answer, from an open semantic API.
function probe(semantic, api, root, spec, config) {
  for (const [file, text] of Object.entries({ 'tsconfig.json': config, ...spec.files })) {
    if (text === undefined) continue;
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), text);
  }
  const configPath = path.join(root, 'tsconfig.json');
  const snapshot = api.updateSnapshot({ openProject: configPath });
  try {
    const project = snapshot.getProject(configPath);
    const { program, checker } = project;
    const name = fileName => {
      const metadata = program.getSourceFileMetadata(fileName);
      if (metadata?.isDefaultLibrary) return `lib:${path.basename(fileName)}`;
      return display(root, fileName) + (metadata?.isFromExternalLibrary ? ' (external)' : '');
    };
    const lines = new Map();
    // Line starts come from the file's text as the compiler reads it, after
    // a leading byte order mark. The text is read from disk, not fetched
    // through the API, whose encoding of a source file fails on some JSON
    // texts (a root `.5`).
    const at = (fileName, offset) => {
      let starts = lines.get(fileName);
      if (!starts) {
        const text = sourceText(fileName);
        lines.set(fileName, starts = lineStarts(text));
      }
      const { line, column } = position(starts, offset);
      return `${line}:${column}`;
    };
    // As `tsc` reports them: a file's binding diagnostics are among its
    // semantic diagnostics, so neither appears for a file TypeScript skips
    // (`SkipTypeChecking`) or on a line a comment directive suppresses.
    const diagnostics = [
      program.getConfigFileParsingDiagnostics(), program.getProgramDiagnostics(), program.getSyntacticDiagnostics(),
      program.getSemanticDiagnostics(),
    ].flat().filter(d => d.code !== 2318).map(d => {
      if (!d.fileName) return `TS${d.code} -`;
      const file = d.fileName.endsWith('.d.ts') && program.getSourceFileMetadata(d.fileName)?.isDefaultLibrary
        ? `lib:${path.basename(d.fileName)}` : display(root, d.fileName);
      return `TS${d.code} ${file}:${at(d.fileName, d.start ?? d.pos)}`;
    });
    const fileNames = program.getSourceFileNames();
    const native = { program: fileNames.map(name), diagnostics };
    if (spec.watch) {
      const watchFile = spec.watchFile ?? (spec.files['main.ts'] !== undefined ? 'main.ts' : 'main.d.ts');
      const sourceFile = program.getSourceFile(path.join(fs.realpathSync(root), watchFile))
        ?? program.getSourceFile(path.join(root, watchFile));
      if (!sourceFile) throw new Error(`${spec.name}: ${watchFile} is not a program file`);
      const identifiers = [];
      const pending = [sourceFile];
      while (pending.length) {
        const node = pending.pop();
        if (node.kind === semantic.SyntaxKind.Identifier && spec.watch.includes(node.text)) identifiers.push(node);
        const children = [];
        node.forEachChild(child => { children.push(child); });
        for (let index = children.length; index--;) pending.push(children[index]);
      }
      native.names = identifiers.map(node => {
        let symbol = checker.getSymbolAtLocation(node);
        if (symbol && symbol.flags & semantic.SymbolFlags.Alias) symbol = checker.getAliasedSymbol(symbol);
        const where = at(sourceFile.fileName, node.getStart(sourceFile));
        if (!symbol || checker.isUnknownSymbol(symbol)) return `${where} ${node.text} -> none`;
        const flags = Object.entries(semantic.SymbolFlags)
          .filter(([, bit]) => typeof bit === 'number' && bit && !(bit & (bit - 1)) && symbol.flags & bit).map(([flag]) => flag);
        const declarations = (symbol.declarations ?? []).map(handle => {
          const kind = semantic.SyntaxKind[handle.kind];
          // Declaration handles name files by the compiler's canonical path,
          // which may be lower-cased; the program file has the real name.
          const fileName = fileNames.find(name => name.toLowerCase() === handle.path.toLowerCase()) ?? handle.path;
          if (program.getSourceFileMetadata(fileName)?.isDefaultLibrary) return `lib:${path.basename(fileName)} ${kind}`;
          // A JSON module's `export=` is declared by the whole file.
          if (handle.kind === semantic.SyntaxKind.SourceFile) return `${display(root, fileName)} ${kind}`;
          const file = program.getSourceFile(fileName);
          return `${display(root, fileName)}:${at(fileName, handle.resolve(project).getStart(file))} ${kind}`;
        });
        return `${where} ${node.text} -> ${flags.join('|')} [${declarations.join(', ')}]`;
      });
    }
    if (spec.showConfig) native.config = showConfig(semantic.binary, root, spec.showConfig);
    return native;
  } finally {
    snapshot.dispose();
  }
}

// `tsc --showConfig` values of `keys`.
function showConfig(binary, root, keys) {
  const result = spawnSync(binary, ['--showConfig', '-p', path.join(root, 'tsconfig.json')], { cwd: root, encoding: 'utf8', timeout: 30_000 });
  if (result.status !== 0) return { exit: result.status };
  const shown = JSON.parse(result.stdout);
  const config = {};
  for (const key of keys) {
    const value = ['files', 'include', 'exclude'].includes(key) ? shown[key] : shown.compilerOptions?.[key];
    config[key] = value === undefined ? null : value;
  }
  return config;
}

// Answer every selected case of `file`; returns the differences from the
// recorded answers and, with `update`, rewrites them.
export function runFixture(semantic, file, { update = false, only } = {}) {
  const fixture = readFixture(file);
  const base = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-json-probes-')));
  const api = new semantic.API({ tsserverPath: semantic.binary, cwd: base });
  const differences = [];
  try {
    const caseProbe = path.join(base, '__JsonProbes_CASE_Probe__');
    for (const spec of fixture.cases) {
      if (only && !only.includes(spec.name)) continue;
      const root = path.join(base, spec.name);
      fs.mkdirSync(root, { recursive: true });
      const native = probe(semantic, api, root, spec, spec.files['tsconfig.json'] === undefined ? fixture.config : undefined);
      // API.updateSnapshot initializes the compiler's canonical filename operation.
      if (typeof api.toPath !== 'function') throw new Error('Pinned API canonical filename operation is missing');
      const caseSensitive = api.toPath(caseProbe) === caseProbe;
      const answers = caseSensitive && spec.caseSensitive ? spec.caseSensitive : spec;
      if (!isDeepStrictEqual(native, answers.native)) {
        differences.push({ file: path.basename(file), name: spec.name, recorded: answers.native ?? null, native });
        if (update) answers.native = native;
      }
    }
  } finally {
    api.close();
    fs.rmSync(base, { recursive: true, force: true });
  }
  if (update && differences.length) fs.writeFileSync(file, `${JSON.stringify(fixture, null, 2)}\n`);
  return differences;
}

async function main(argv) {
  const update = argv.includes('--update');
  const only = argv.find(arg => arg.startsWith('--only='))?.slice('--only='.length).split(',');
  const unknown = argv.filter(arg => arg.startsWith('--') && arg !== '--update' && !arg.startsWith('--only='));
  if (unknown.length) throw new Error(`unknown option ${unknown[0]}`);
  const files = argv.filter(arg => !arg.startsWith('--'));
  const semantic = await loadSemanticApi();
  let count = 0;
  for (const file of files.length ? files.map(f => path.resolve(f)) : fixtureFiles()) {
    const differences = runFixture(semantic, file, { update, only });
    for (const difference of differences) {
      count++;
      console.log(`${update ? 'updated' : 'differs'}: ${difference.file} ${difference.name}`);
      if (!update) console.log(JSON.stringify({ recorded: difference.recorded, native: difference.native }, null, 2));
    }
  }
  if (count === 0) console.log('json probes: every recorded answer agrees with the compiler');
  else if (!update) process.exitCode = 1;
}

if (process.argv[1] && fs.realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch(error => {
    console.error(error.stack ?? String(error));
    process.exitCode = 1;
  });
}

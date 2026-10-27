// Compare the inputs this tool selects for a real project with TypeScript
// 7.0.2's program: the same files, in `tsc --listFilesOnly` order, with the
// same provenance as the semantic API's `getSourceFileMetadata`. With
// `--oracle=5.6`, compare them with TypeScript 5.6.3's program instead, for
// the TypeScript 5.6 dialect's acceptance.
//
//   TSPARSE_PROGRAM_CONFIG=/abs/app/tsconfig.json \
//   TSPARSE_PROGRAM_LISTING=/abs/listing.json \
//     cargo test --offline --release --lib -- --ignored --exact \
//       project::program::tests::list_configured_program
//   TYPE_SCRIPT_SEMANTIC_API=/path/to/typescript-7.0.2/package \
//   TYPE_SCRIPT_NATIVE=/path/to/typescript-<platform>-7.0.2/package/lib/tsc \
//     node tools/program-inputs.mjs /abs/listing.json /abs/app/tsconfig.json [--report=PATH]
//   [TYPE_SCRIPT_56_API=/path/to/typescript-5.6.3/package] \
//     node tools/program-inputs.mjs /abs/listing.json /abs/app/tsconfig.json --oracle=5.6 [--report=PATH]
//
// The Rust test writes the tool's listing: each input in program order, a
// vendored standard library by its file name and any other file relative to
// the configuration's directory, with its provenance (`library`,
// `dependency`, or `project`, the searched files), and each file that
// redirects to an input as a duplicate of its package (`redirects`, `file`
// and `target` named the same way). This runner names the
// compiler's files the same way (a library from the compiler's own directory
// by its file name, any other file by its canonical path), takes the
// compiler's order from `tsc --listFilesOnly`, checks that the semantic API
// lists the same order, and classifies each file as a `library`
// (`isDefaultLibrary`), a `dependency` (`isFromExternalLibrary`), or a
// `project` file. It prints a summary, writes the complete comparison to the
// report (default `.cache/program-inputs.json`), and exits 1 on any
// difference in the file set, order, or provenance, and 2 when it cannot run.
//
// `--oracle=5.6` takes the pinned 5.6.3 package from TYPE_SCRIPT_56_API or the
// canonical checkout (see `ts56-compiler.mjs`), the order from its
// `lib/tsc.js -p CONFIG --listFilesOnly --incremental false`, and the
// provenance from its API's program (`incremental` forced false; 5.6.3 would
// otherwise write `.tsbuildinfo` into the project): `isSourceFileDefaultLibrary`,
// `isSourceFileFromExternalLibrary`, or neither. 5.6.3 lists a package-id
// duplicate, a SourceFile with `redirectInfo`, as its own input where 7.0.2
// lists only the target, so a redirect counts as a spelling of
// `redirectInfo.redirectTarget`: it leaves the order and the file set, the
// target keeps its own position (5.6.3 lists a redirect met while its
// target's imports are processed before the target), and the report lists
// each pair under `redirects`. A listing's own `redirects` must then name the
// same files with the same targets (a `redirect` difference otherwise). The
// default report is `.cache/program-inputs-5.6.json`.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { loadSemanticApi } from './semantic-oracle.mjs';
import { loadTypeScript56, parseConfig, programFileName, TSC_ARGUMENTS } from './ts56-compiler.mjs';

const options = process.argv.slice(2);
const reportOption = options.find(option => option.startsWith('--report='));
const oracleOption = options.find(option => option.startsWith('--oracle='));
const operands = options.filter(option => option !== reportOption && option !== oracleOption);
const oracle = oracleOption?.slice('--oracle='.length);
if (operands.length !== 2 || operands.some(operand => operand.startsWith('--')) || (oracle !== undefined && oracle !== '5.6')) {
  console.error('usage: node tools/program-inputs.mjs LISTING.json TSCONFIG [--oracle=5.6] [--report=PATH]');
  process.exit(2);
}
const reportPath = reportOption?.slice('--report='.length)
  ?? (oracle === '5.6' ? '.cache/program-inputs-5.6.json' : '.cache/program-inputs.json');

try {
  const [listingPath, configArgument] = operands;
  const tool = JSON.parse(fs.readFileSync(listingPath, 'utf8'));
  const config = fs.realpathSync(path.resolve(configArgument));
  if (path.resolve(tool.config) !== config) throw new Error(`${listingPath} lists ${tool.config}, not ${config}`);
  const report = await compare(tool.files, config, tool.redirects);
  fs.mkdirSync(path.dirname(path.resolve(reportPath)), { recursive: true });
  fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 1)}\n`);
  const { files, ...summary } = report;
  console.log(JSON.stringify(summary, null, 2));
  process.exitCode = report.differences.length ? 1 : 0;
} catch (error) {
  console.error(`program-inputs: ${error.message}`);
  process.exit(2);
}

async function compare(toolFiles, config, toolRedirects) {
  const root = path.dirname(config);
  const { compiler, files: compilerFiles, redirects } = oracle === '5.6' ? program56(config, root) : await program702(config, root);

  const differences = [];
  const toolOrigin = new Map(toolFiles.map(file => [file.file, file.origin]));
  const compilerOrigin = new Map(compilerFiles.map(file => [file.file, file.origin]));
  for (const file of compilerFiles) {
    const origin = toolOrigin.get(file.file);
    if (origin === undefined) differences.push({ kind: 'onlyCompiler', file: file.file });
    else if (origin !== file.origin) differences.push({ kind: 'provenance', file: file.file, tool: origin, compiler: file.origin });
  }
  for (const file of toolFiles) if (!compilerOrigin.has(file.file)) differences.push({ kind: 'onlyTool', file: file.file });
  if (redirects && toolRedirects) {
    const toolTarget = new Map(toolRedirects.map(redirect => [redirect.file, redirect.target]));
    const compilerTarget = new Map(redirects.map(redirect => [redirect.file, redirect.target]));
    for (const file of new Set([...compilerTarget.keys(), ...toolTarget.keys()])) {
      if (toolTarget.get(file) !== compilerTarget.get(file))
        differences.push({ kind: 'redirect', file, tool: toolTarget.get(file) ?? null, compiler: compilerTarget.get(file) ?? null });
    }
  }
  const order = compilerFiles.map(file => file.file);
  const length = Math.max(toolFiles.length, order.length);
  for (let index = 0; index < length; index++) {
    if (toolFiles[index]?.file !== order[index]) differences.push({ kind: 'order', index, tool: toolFiles[index]?.file, compiler: order[index] });
  }
  const counts = {};
  for (const file of toolFiles) {
    const kind = file.origin === 'library' ? 'library'
      : /\.json$/.test(file.file) ? 'json'
      : /\.d\.([cm]?ts|[^/]*\.ts)$/.test(file.file) ? 'declaration'
      : /\.tsx$/.test(file.file) ? 'tsx' : 'ts';
    const key = `${file.origin} ${kind}`;
    counts[key] = (counts[key] ?? 0) + 1;
  }
  const byKind = {};
  for (const difference of differences) byKind[difference.kind] = (byKind[difference.kind] ?? 0) + 1;
  return {
    config,
    compiler,
    tool: toolFiles.length,
    compilerFiles: compilerFiles.length,
    counts,
    byKind,
    differences: differences.slice(0, 200),
    ...(redirects && { redirects }),
    files: compilerFiles,
  };
}

// TypeScript 7.0.2: the native `tsc --listFilesOnly` order, which the semantic
// API must list too, and the API's `getSourceFileMetadata` provenance.
async function program702(config, root) {
  const semantic = await loadSemanticApi();
  const name = programFileName(root, fs.realpathSync(path.dirname(semantic.binary)));
  const listed = spawnSync(semantic.binary, ['-p', config, '--listFilesOnly'], { cwd: root, encoding: 'utf8', maxBuffer: 1 << 28 });
  if (listed.error || listed.status !== 0) throw new Error(`tsc --listFilesOnly failed: ${listed.error?.message || listed.stderr.trim() || `exit ${listed.status}`}`);
  const order = listed.stdout.split('\n').filter(Boolean).map(name);

  const api = new semantic.API({ tsserverPath: semantic.binary, cwd: root });
  let compilerFiles;
  try {
    const snapshot = api.updateSnapshot({ openProject: config });
    try {
      const { program } = snapshot.getProject(config);
      compilerFiles = program.getSourceFileNames().map(fileName => {
        const metadata = program.getSourceFileMetadata(fileName);
        const origin = metadata.isDefaultLibrary ? 'library' : metadata.isFromExternalLibrary ? 'dependency' : 'project';
        return { file: name(fileName), origin };
      });
    } finally {
      snapshot.dispose();
    }
  } finally {
    api.close();
  }
  if (compilerFiles.map(file => file.file).join('\n') !== order.join('\n'))
    throw new Error('the semantic API and tsc --listFilesOnly list different programs');
  return { compiler: semantic.nativeVersion, files: compilerFiles };
}

// TypeScript 5.6.3: its `tsc --listFilesOnly --incremental false` order, which
// its API's program must list too, and the program's provenance, with each
// redirect folded into its target.
function program56(config, root) {
  const { ts, tsc, libraries } = loadTypeScript56();
  const name = programFileName(root, libraries);
  const listed = spawnSync(process.execPath, [tsc, '-p', config, '--listFilesOnly', ...TSC_ARGUMENTS],
    { cwd: root, encoding: 'utf8', maxBuffer: 1 << 28 });
  if (listed.error || listed.status !== 0) {
    // 5.6.3 writes its diagnostics to stdout among the listed files.
    const diagnostics = (listed.stdout ?? '').split('\n').filter(line => /\berror TS\d+:/.test(line)).join('; ');
    throw new Error(`TypeScript 5.6.3 tsc --listFilesOnly failed: ${listed.error?.message || listed.stderr.trim() || diagnostics || `exit ${listed.status}`}`);
  }

  const parsed = parseConfig(ts, config);
  const program = ts.createProgram({
    rootNames: parsed.fileNames, options: parsed.options, projectReferences: parsed.projectReferences,
    configFileParsingDiagnostics: ts.getConfigFileParsingDiagnostics(parsed),
  });
  const origin = sourceFile => program.isSourceFileDefaultLibrary(sourceFile) ? 'library'
    : program.isSourceFileFromExternalLibrary(sourceFile) ? 'dependency' : 'project';
  const redirects = [];
  const targetOf = new Map();
  const entries = program.getSourceFiles().map(sourceFile => {
    const target = sourceFile.redirectInfo?.redirectTarget ?? sourceFile;
    const file = name(sourceFile.fileName);
    if (target !== sourceFile) {
      const redirect = { file, target: name(target.fileName) };
      redirects.push(redirect);
      targetOf.set(redirect.file, redirect.target);
    }
    return { file, origin: origin(target) };
  });
  const files = foldRedirects(entries, targetOf);
  const order = foldRedirects(listed.stdout.split('\n').filter(Boolean).map(fileName => ({ file: name(fileName) })), targetOf);
  if (files.map(file => file.file).join('\n') !== order.map(file => file.file).join('\n'))
    throw new Error('the TypeScript 5.6.3 API and tsc --listFilesOnly list different programs');
  return { compiler: `Version ${ts.version}`, files, redirects };
}

// Entries without the redirect spellings (`targetOf`): each target keeps its
// own position, where 7.0.2 lists it, even where 5.6.3 lists a redirect
// before its target (met while the target's own imports were processed).
function foldRedirects(entries, targetOf) {
  return entries.filter(entry => !targetOf.has(entry.file));
}

// Compare the inputs this tool selects for a real project with TypeScript
// 7.0.2's program: the same files, in `tsc --listFilesOnly` order, with the
// same provenance as the semantic API's `getSourceFileMetadata`.
//
//   TSPARSE_PROGRAM_CONFIG=/abs/app/tsconfig.json \
//   TSPARSE_PROGRAM_LISTING=/abs/listing.json \
//     cargo test --offline --release --lib -- --ignored --exact \
//       project::program::tests::list_configured_program
//   TYPE_SCRIPT_SEMANTIC_API=/path/to/typescript-7.0.2/package \
//   TYPE_SCRIPT_NATIVE=/path/to/typescript-<platform>-7.0.2/package/lib/tsc \
//     node tools/program-inputs.mjs /abs/listing.json /abs/app/tsconfig.json [--report=PATH]
//
// The Rust test writes the tool's listing: each input in program order, a
// vendored standard library by its file name and any other file relative to
// the configuration's directory, with its provenance (`library`,
// `dependency`, or `project`, the searched files). This runner names the
// compiler's files the same way (a library from the compiler's own directory
// by its file name, any other file by its canonical path), takes the
// compiler's order from `tsc --listFilesOnly`, checks that the semantic API
// lists the same order, and classifies each file as a `library`
// (`isDefaultLibrary`), a `dependency` (`isFromExternalLibrary`), or a
// `project` file. It prints a summary, writes the complete comparison to the
// report (default `.cache/program-inputs.json`), and exits 1 on any
// difference in the file set, order, or provenance, and 2 when it cannot run.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { loadSemanticApi } from './semantic-oracle.mjs';

const options = process.argv.slice(2);
const reportOption = options.find(option => option.startsWith('--report='));
const operands = options.filter(option => option !== reportOption);
if (operands.length !== 2 || operands.some(operand => operand.startsWith('--'))) {
  console.error('usage: node tools/program-inputs.mjs LISTING.json TSCONFIG [--report=PATH]');
  process.exit(2);
}
const reportPath = reportOption?.slice('--report='.length) ?? '.cache/program-inputs.json';

try {
  const [listingPath, configArgument] = operands;
  const tool = JSON.parse(fs.readFileSync(listingPath, 'utf8'));
  const config = fs.realpathSync(path.resolve(configArgument));
  if (path.resolve(tool.config) !== config) throw new Error(`${listingPath} lists ${tool.config}, not ${config}`);
  const report = await compare(tool.files, config);
  fs.mkdirSync(path.dirname(path.resolve(reportPath)), { recursive: true });
  fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 1)}\n`);
  const { files, ...summary } = report;
  console.log(JSON.stringify(summary, null, 2));
  process.exitCode = report.differences.length ? 1 : 0;
} catch (error) {
  console.error(`program-inputs: ${error.message}`);
  process.exit(2);
}

async function compare(toolFiles, config) {
  const root = path.dirname(config);
  const semantic = await loadSemanticApi();
  const compilerLibraries = fs.realpathSync(path.dirname(semantic.binary));
  const name = fileName => {
    const real = fs.realpathSync(fileName);
    if (path.dirname(real) === compilerLibraries) return path.basename(real);
    return path.relative(root, real).split(path.sep).join('/');
  };
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

  const differences = [];
  const toolOrigin = new Map(toolFiles.map(file => [file.file, file.origin]));
  const compilerOrigin = new Map(compilerFiles.map(file => [file.file, file.origin]));
  for (const file of compilerFiles) {
    const origin = toolOrigin.get(file.file);
    if (origin === undefined) differences.push({ kind: 'onlyCompiler', file: file.file });
    else if (origin !== file.origin) differences.push({ kind: 'provenance', file: file.file, tool: origin, compiler: file.origin });
  }
  for (const file of toolFiles) if (!compilerOrigin.has(file.file)) differences.push({ kind: 'onlyTool', file: file.file });
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
    compiler: semantic.nativeVersion,
    tool: toolFiles.length,
    compilerFiles: compilerFiles.length,
    counts,
    byKind,
    differences: differences.slice(0, 200),
    files: compilerFiles,
  };
}

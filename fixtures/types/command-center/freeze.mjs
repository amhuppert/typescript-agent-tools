import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { isDeepStrictEqual } from 'node:util';
import { loadSemanticApi, lineStarts, position } from '../../../tools/semantic-oracle.mjs';
import { compilerText } from '../../../tools/navigation-oracle.mjs';
import { captureFileSystem } from '../../../tools/type-oracle-filesystem.mjs';

const args = process.argv.slice(2);
const usage = 'Usage: node fixtures/types/command-center/freeze.mjs CORPUS OUTPUT [--revision=FULL_SHA] [--config=RELATIVE_JSON]';
if (args.length < 2 || args.length > 4) throw Error(usage);
const options = new Map();
for (const argument of args.slice(2)) {
  const matched = /^(--revision|--config)=(.+)$/.exec(argument);
  if (!matched || options.has(matched[1])) throw Error(usage);
  options.set(matched[1],matched[2]);
}
const root = fs.realpathSync(path.resolve(args[0]));
const output = path.resolve(args[1]);
const expectedRevision = options.get('--revision') ?? '92fe6ab06fbea2c4b6a305689f8cf336173eef81';
if (!/^[0-9a-f]{40}$/.test(expectedRevision)) throw Error(usage);
const configName = options.get('--config') ?? 'tsconfig.json';
if (path.isAbsolute(configName) || path.relative(root,path.resolve(root,configName)).startsWith('..'))
  throw Error('Corpus configuration must remain inside the isolated root');
const git = (...args) => execFileSync('git', ['-C', root, ...args], { encoding: 'utf8' });
function verifyRevision() {
  if (git('rev-parse', 'HEAD').trim() !== expectedRevision) throw Error('Corpus revision differs from expected revision');
  git('diff', '--quiet', 'HEAD', '--');
}
verifyRevision();
if (!process.env.TYPE_SCRIPT_SEMANTIC_API) throw Error('Freeze requires TYPE_SCRIPT_SEMANTIC_API for exact package identity');
const apiDirectory = fs.realpathSync(path.resolve(process.env.TYPE_SCRIPT_SEMANTIC_API));
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const rawInput = file => {
  const bytes = fs.readFileSync(path.resolve(root, file));
  return { file, sha256: hash(bytes), bytes: bytes.length };
};
const configuration = rawInput(configName);
const locks = ['package.json', 'package-lock.json', 'pnpm-lock.yaml', 'yarn.lock', 'bun.lock', 'bun.lockb', 'node_modules/.package-lock.json']
  .filter(file => fs.existsSync(path.join(root, file))).map(rawInput);
const semantic = await loadSemanticApi({ api: apiDirectory });
const compiler = { version: semantic.version, nativeVersion: semantic.nativeVersion,
  binary: semantic.binary, binarySha256: hash(fs.readFileSync(semantic.binary)),
  apiPackage: path.join(apiDirectory, 'package.json'), apiPackageSha256: hash(fs.readFileSync(path.join(apiDirectory, 'package.json'))) };
const tracked = new Set(git('ls-files', '-z').split('\0').filter(Boolean));
// The replay capture accepts UTF-8 inputs and preserves their raw BOM bytes.
// Reject another encoding instead of hashing a lossy replacement projection.
const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
const selectors = [
  ['annotated-member', 'src/lib/specs/execution-service.ts', 'SpecWorkflowCleanupPort.observe', '  observe(', 'value'],
  ['contextual-member', 'src/lib/specs/workflow-cleanup-port.ts', null, '    observe:', 'value'],
  ['inferred-factory', 'src/lib/agent-gateway/route-handlers.ts', 'createAgentGatewayHandlers', 'export function createAgentGatewayHandlers', 'value'],
  ['jsx-member', 'src/components/CollapsibleText.tsx', 'CollapsibleTextProps.maxCollapsedHeight', '  maxCollapsedHeight?:', 'value'],
  ['predicate-callback', 'src/lib/notepads/service.ts', null, '  return modes.filter(', 'expression'],
];
function discover() {
  const capture = captureFileSystem();
  const api = new semantic.API({ tsserverPath: semantic.binary, cwd: root, fs: capture.callbacks });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProject: path.join(root, configuration.file) });
    const project = snapshot.getProject(path.join(root, configuration.file));
    if (!project) throw Error('No corpus project');
    const program = project.program;
    const failures = [...program.getConfigFileParsingDiagnostics(), ...program.getProgramDiagnostics()].filter(d => d.category === 1);
    if (failures.length) throw Error(JSON.stringify(failures));
    const files = program.getSourceFileNames().map(name => {
      const relative = path.relative(root, name).split(path.sep).join('/');
      const bytes = fs.readFileSync(name);
      if (compilerText(decoder.decode(bytes)) !== program.getSourceFile(name).text)
        throw Error('Corpus source differs from compiler snapshot: ' + relative);
      const metadata = program.getSourceFileMetadata(name);
      return { file: relative, sha256: hash(bytes), bytes: bytes.length, tracked: tracked.has(relative),
        defaultLibrary: metadata.isDefaultLibrary, externalLibrary: metadata.isFromExternalLibrary };
    });
    const programFiles = files.map(row => row.file);
    const sources = new Map(files.map(row => [row.file, row]));
    const queries = selectors.map(([id, file, selector, needle, question]) => {
      const text = program.getSourceFile(path.join(root, file)).text;
      const offset = text.indexOf(needle);
      if (offset < 0 || text.indexOf(needle, offset + 1) >= 0) throw Error('Nonunique anchor ' + id);
      const skip = needle.search(/observe|createAgentGatewayHandlers|maxCollapsedHeight|filter/);
      return { id, selector, at: { file, ...position(lineStarts(text), offset + skip) }, question, sourceSha256: sources.get(file).sha256 };
    });
    files.sort((a, b) => a.file < b.file ? -1 : a.file > b.file ? 1 : 0);
    capture.verify();
    return { files, programFiles, queries, resolvedConfigOptions: program.getCompilerOptions() };
  } finally { snapshot?.dispose(); api.close(); }
}
const discovered = discover();
// A second fresh program detects added/removed selected files as well as changes
// to contents, order and provenance. This is verification, not atomic capture.
if (!isDeepStrictEqual(discovered, discover())) throw Error('Corpus program changed during discovery');
verifyRevision();
for (const row of [configuration, ...locks, ...discovered.files])
  if (rawInput(row.file).sha256 !== row.sha256) throw Error('Corpus input changed: ' + row.file);
if (hash(fs.readFileSync(compiler.binary)) !== compiler.binarySha256
    || hash(fs.readFileSync(compiler.apiPackage)) !== compiler.apiPackageSha256)
  throw Error('Compiler identity changed during discovery');
const result = { schema: 2, repository: 'command-center', revision: expectedRevision,
  physicalRoot: root, currentDirectory: root, rawSourceEncoding: 'UTF-8', programRediscovered: true,
  configuration: { ...configuration, resolvedConfigOptions: discovered.resolvedConfigOptions,
    optionsNote: 'Inherited/configuration values returned by getCompilerOptions; strict-family and other operation defaults are not separately materialized by this API.' },
  compiler, locks, queries: discovered.queries, programFiles: discovered.programFiles, files: discovered.files,
  generatedInputs: discovered.files.filter(file => !file.tracked && !file.externalLibrary && !file.defaultLibrary) };
fs.writeFileSync(output, JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify({ revision: result.revision, physicalRoot: root, files: result.files.length,
  queries: result.queries, generatedInputs: result.generatedInputs.length, locks }));

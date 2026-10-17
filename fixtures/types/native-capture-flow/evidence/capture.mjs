import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { typeOracle } from '../../../../tools/type-oracle.mjs';
import { lineStarts, position } from '../../../../tools/semantic-oracle.mjs';

const root = process.cwd(), evidence = path.join(root, 'fixtures/types/native-capture-flow/evidence');
const apiRoot = process.env.TYPE_SCRIPT_SEMANTIC_API;
const binary = process.env.TYPE_SCRIPT_NATIVE;
if (!apiRoot || !binary) throw new Error('Capture needs explicit pinned semantic API and native compiler');
const common = spawnSync('git', ['rev-parse', '--path-format=absolute', '--git-common-dir'], { encoding: 'utf8' });
if (common.error || common.status !== 0) throw new Error('Cannot locate common repository');
const nativeSource = path.join(path.dirname(common.stdout.trim()), '.cache/typescript-go-7.0.2');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const row = (file, relativeRoot = root) => {
  const bytes = fs.readFileSync(file);
  return { file: path.relative(relativeRoot, file).split(path.sep).join('/'), bytes: bytes.length, sha256: digest(bytes) };
};
const json = (name, value) => fs.writeFileSync(path.join(evidence, name), JSON.stringify(value, null, 2) + '\n');
const git = args => {
  const r = spawnSync('git', ['-C', nativeSource, ...args], { encoding: 'utf8' });
  if (r.error || r.status !== 0) throw new Error(`Pinned source command failed: ${r.error?.message || r.stderr}`);
  return r.stdout.trim();
};
if (git(['rev-parse', 'HEAD']) !== '2bd066d87f5bafd315be9f40889d0a60b9e58e0b' || git(['status', '--porcelain']))
  throw new Error('Compiler source is not exact pristine 7.0.2');
const live = [
  'tools/type-capture-flow-oracle.test.mjs', 'tools/type-capture-boundaries-oracle.test.mjs',
  'tools/type-oracle.mjs', 'tools/semantic-oracle.mjs', 'tools/navigation-oracle.mjs',
  'tools/corpus-selection.mjs', 'tools/vendor-typescript-libs.mjs',
  ...['flow.ts', 'tsconfig.json', 'loose.json'].map(f => `fixtures/types/native-capture-flow/${f}`),
  ...['boundaries.ts', 'tsconfig.json', 'loose.json'].map(f => `fixtures/types/native-capture-boundaries/${f}`),
];
const before = live.map(file => row(path.join(root, file)));
const captureBefore = row(path.join(evidence, 'capture.mjs'));
fs.mkdirSync(path.join(evidence, 'captured-tooling'), { recursive: true });
for (const file of live) {
  const destination = file.startsWith('tools/')
    ? path.join(evidence, 'captured-tooling', path.basename(file))
    : path.join(evidence, 'inputs', file.replace('fixtures/types/', ''));
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.copyFileSync(path.join(root, file), destination);
}
for (const [source, destination] of [
  ['.cc/temp/types-capture-flow/fresh.json', 'historical-capture-flow.json'],
  ['.cc/temp/types-capture-boundaries/fresh.json', 'historical-capture-boundaries.json'],
]) fs.copyFileSync(path.join(root, source), path.join(evidence, destination));
const inventory = [];
const visit = (directory, rows = inventory) => {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true }).sort((a,b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0)) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) visit(file, rows);
    else if (entry.isFile()) rows.push(row(file, apiRoot));
    else throw new Error(`Unsupported installed API entry ${file}`);
  }
};
visit(apiRoot);
json('installed-api-inventory.json', inventory);
const nativeFiles = ['internal/checker/flow.go', 'internal/checker/checker.go', 'internal/ast/utilities.go'];
const nativeBefore = nativeFiles.map(file => row(path.join(nativeSource, file), nativeSource));
const binaryBefore = row(binary, path.dirname(binary));
const anchors = [
  ['internal/checker/flow.go', 'func (c *Checker) isSymbolAssignedDefinitely', 'func (c *Checker) extendAssignmentPosition'],
  ['internal/checker/checker.go', '\t// The declaration container is the innermost function', '\t// A variable is considered uninitialized'],
];
for (const [file, start, end] of anchors) {
  const text = fs.readFileSync(path.join(nativeSource, file), 'utf8');
  const first = text.indexOf(start), last = text.indexOf(end, first);
  if (first < 0 || last < first) throw new Error(`Missing pinned anchor in ${file}`);
  // Capture the last function completely where it is the final file function.
  const slice = file.endsWith('flow.go') ? text.slice(first) : text.slice(first, last);
  fs.writeFileSync(path.join(evidence, path.basename(file) + '.anchors.txt'), slice);
}
const utilities = fs.readFileSync(path.join(nativeSource, 'internal/ast/utilities.go'), 'utf8');
const segments = [
  ['func IsFunctionLikeKind', 'func IsFunctionLike('],
  ['func IsFunctionOrSourceFile', 'func IsClassLike'],
  ['func IsTypeNodeKind', 'func IsJSDocKind'],
  ['func FindAncestor(', 'func FindAncestorKind'],
];
fs.writeFileSync(path.join(evidence, 'utilities.go.anchors.txt'), segments.map(([a,b]) => {
  const first = utilities.indexOf(a), last = utilities.indexOf(b, first);
  if (first < 0 || last < first) throw new Error(`Missing utilities anchor ${a}`);
  return utilities.slice(first, last);
}).join('\n'));
const observations = [];
async function demand(directory, config, text, file, marker, diagnostics = false, variant = undefined) {
  const label = '/*' + marker + '*/ ';
  const offset = text.indexOf(label);
  if (offset < 0) throw new Error(`Missing marker ${marker}`);
  const request = { at: { file, ...position(lineStarts(text), offset + label.length) }, properties: false, ...(diagnostics ? { diagnostics: true } : {}) };
  observations.push({ family: file, config, marker, ...(variant ? { variant } : {}), request,
    answer: await typeOracle(path.join(directory, config), request) });
}
const flowDirectory = path.join(root, 'fixtures/types/native-capture-flow');
const flow = fs.readFileSync(path.join(flowDirectory, 'flow.ts'), 'utf8');
const flowMarkers = [...flow.matchAll(/\/\*([^*]+)\*\/ /g)].map(match => match[1]);
for (const config of ['tsconfig.json', 'loose.json']) {
  for (const marker of flowMarkers) await demand(flowDirectory, config, flow, 'flow.ts', marker);
  for (const marker of ['never-initialized', 'compound-not-definite', 'definitely-assigned-before', 'definitely-assigned-after'])
    await demand(flowDirectory, config, flow, 'flow.ts', marker, true);
}
const boundaryDirectory = path.join(root, 'fixtures/types/native-capture-boundaries');
const boundaries = fs.readFileSync(path.join(boundaryDirectory, 'boundaries.ts'), 'utf8');
const boundaryMarkers = [...boundaries.matchAll(/\/\*([^*]+)\*\/ /g)].map(match => match[1]);
for (const config of ['tsconfig.json', 'loose.json'])
  for (const marker of boundaryMarkers) await demand(boundaryDirectory, config, boundaries, 'boundaries.ts', marker);
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'capture-frozen-'));
try {
  for (const lineBreak of ['\n', '\r', '\r\n', '\u2028', '\u2029']) for (const bom of ['', '\ufeff']) {
    const accepted = boundaries.replaceAll('\n', lineBreak), disk = bom + accepted;
    fs.writeFileSync(path.join(temporary, 'boundaries.ts'), disk);
    for (const config of ['tsconfig.json', 'loose.json']) fs.copyFileSync(path.join(boundaryDirectory, config), path.join(temporary, config));
    for (const config of ['tsconfig.json', 'loose.json'])
      for (const marker of ['variable-inside', 'variable-after', 'if-inside', 'class-inside', 'class-after', 'nested-declaration', 'shadow-read'])
        await demand(temporary, config, accepted, 'boundaries.ts', marker, false,
          { lineBreak: [...lineBreak].map(c => c.codePointAt(0)), bom: !!bom, diskSha256: digest(Buffer.from(disk)) });
  }
} finally { fs.rmSync(temporary, { recursive: true, force: true }); }
json('fresh-observations.json', observations);
const runs = [];
for (const suite of ['tools/type-capture-flow-oracle.test.mjs', 'tools/type-capture-boundaries-oracle.test.mjs']) {
  const started = new Date().toISOString();
  const r = spawnSync('cctl', ['validate', 'run', 'harness-test', '--queue-if-busy', '--require-match', '--', suite], { encoding: 'utf8', timeout: 600000 });
  runs.push({ suite, started, finished: new Date().toISOString(), status: r.status, signal: r.signal, error: r.error?.message, stdout: r.stdout, stderr: r.stderr });
  json('registered-replays.json', runs);
  process.stdout.write(r.stdout); process.stderr.write(r.stderr);
  if (r.error || r.signal || r.status !== 0) throw new Error(`Registered frozen replay failed: ${suite}`);
}
const after = live.map(file => row(path.join(root, file)));
const nativeAfter = nativeFiles.map(file => row(path.join(nativeSource, file), nativeSource));
const binaryAfter = row(binary, path.dirname(binary));
const captureAfter = row(path.join(evidence, 'capture.mjs'));
const apiAfter = [];
visit(apiRoot, apiAfter);
if (JSON.stringify(before) !== JSON.stringify(after) || JSON.stringify(nativeBefore) !== JSON.stringify(nativeAfter)
  || JSON.stringify(binaryBefore) !== JSON.stringify(binaryAfter) || JSON.stringify(inventory) !== JSON.stringify(apiAfter)
  || JSON.stringify(captureBefore) !== JSON.stringify(captureAfter) || git(['status', '--porcelain']))
  throw new Error('Captured source/tool/binary inputs changed during replay');
json('replay-provenance.json', {
  kind: 'fresh hash-frozen API observations and registered single-file replays; historical observations have separate weaker provenance',
  compiler: '7.0.2', commit: git(['rev-parse', 'HEAD']), nativeSourceStatus: 'pristine including untracked files',
  before, after, nativeBefore, nativeAfter, binaryBefore, binaryAfter, installedApi: apiRoot, installedApiInventory: 'installed-api-inventory.json',
  captureBefore, captureAfter, installedApiAfterEqualsBefore: true,
  observations: observations.length, phases: { flowDemandOnly: 66, flowPostDemandDiagnostics: 8, boundaryDemandOnly: 54, textVariantsDemandOnly: 140 },
  provenanceBoundary: 'API answer sourceHash is accepted compiler text; variant diskSha256 preserves BOM bytes. Installed API inventory is separate from per-request compiler-read metadata. No historical metadata manifest or before/after hashes are retroactively asserted.',
});

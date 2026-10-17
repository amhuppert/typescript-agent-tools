// Data-only evidence wiring. Run with the registered compiler environment:
// source scripts/validate/common.sh; find_semantic_compilers
// node fixtures/types/freeze-oracle-manifest.mjs
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { loadSemanticApi, lineStarts, position } from '../../tools/semantic-oracle.mjs';
import { sources } from '../../tools/vendor-typescript-libs.mjs';
import { crossProductSource, headRecursiveSource, instantiationCountSource,
  relationDepthSource, subtypeRemovalSource, relationMaybeSource, relationAllowanceSource } from './compiler-limits/generate.mjs';

const directory = path.dirname(fileURLToPath(import.meta.url));
const repository = path.resolve(directory, '../..');
const relative = file => path.relative(repository, file).split(path.sep).join('/');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const record = file => { const bytes = fs.readFileSync(file); return { file: relative(file), bytes: bytes.length, sha256: digest(bytes) }; };
const inventory = [];
function visit(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    if (['command-center', 'representation', 'relation-research', 'captured-tooling', 'oracle-manifest.json'].includes(entry.name)) continue;
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) visit(file);
    else if (entry.isFile()) inventory.push(record(file));
    else throw new Error(`Unexpected fixture filesystem entry: ${file}`);
  }
}
visit(directory);
inventory.push(...['tools/type-oracle.mjs', 'tools/semantic-oracle.mjs', 'tools/navigation-oracle.mjs',
  'tools/type-oracle.test.mjs', 'tools/type-oracle-review.test.mjs', 'tools/type-selection-oracle-review.test.mjs',
  'tools/type-member-relations.test.mjs', 'tools/type-algebra-oracle.test.mjs', 'tools/type-json-oracle.test.mjs',
  'tools/type-native-vertical-oracle.test.mjs']
  .map(file => record(path.join(repository, file))));
inventory.sort((a, b) => a.file < b.file ? -1 : a.file > b.file ? 1 : 0);
const anchors = [];
for (const item of inventory.filter(item => /\.[cm]?tsx?$/.test(item.file))) {
  const source = fs.readFileSync(path.join(repository, item.file), 'utf8');
  const starts = lineStarts(source);
  for (const match of source.matchAll(/\/\*#([^*]+)\*\//g)) anchors.push({ file: item.file, marker: match[1],
    ...position(starts, match.index + match[0].length), sourceSha256: item.sha256 });
}
const semantic = await loadSemanticApi();
const configurations = [];
// These are the fixture configurations used by the executable suites; JSON data
// modules and provenance documents are inputs, not project configurations.
for (const item of inventory.filter(item => /\/(?:tsconfig|base|loose|strict-[01]-exact-[01]-unchecked-[01])\.json$/.test(item.file))) {
  const file = path.join(repository, item.file);
  const api = new semantic.API({ tsserverPath: semantic.binary, cwd: path.dirname(file) });
  let snapshot;
  try {
    snapshot = api.updateSnapshot({ openProject: file });
    const project = snapshot.getProject(file);
    if (!project) throw new Error(`Compiler did not open ${file}`);
    configurations.push({ file: item.file, sourceSha256: item.sha256, compilerOptions: project.program.getCompilerOptions() });
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}
const generated = [...[47, 48, 49].map(length => ({ family: 'OuterHead', length, source: headRecursiveSource(length) })),
  ...[2, 3, 4].map(identityAliases => ({ family: 'instantiationCount', identityAliases, source: instantiationCountSource(identityAliases) })),
  ...[[3, 33333], [100, 1000], [11, 9091]].map(([left, right]) => ({ family: 'crossProduct', left, right, source: crossProductSource(left, right) })),
  ...[99, 100, 101].map(depth => ({ family: 'relationDepth', depth, leaf: 'same', source: relationDepthSource(depth) })),
  { family: 'relationDepth', depth: 99, leaf: 'different', source: relationDepthSource(99, 'different') },
  ...[43, 42, 41].map(removals => ({ family: 'subtypeRemoval', length: 1250, removals, source: subtypeRemovalSource(1250, removals) })),
  ...['active', 'active-false', 'deep'].map(mode => ({ family: 'relationMaybe', mode, source: relationMaybeSource(mode) })),
  ...[1998, 1999, 2000].map(length => ({ family: 'relationAllowance', length, source: relationAllowanceSource(length) }))]
  .map(({ source, ...parameters }) => ({ ...parameters, bytes: Buffer.byteLength(source), sha256: digest(source),
    anchor: position(lineStarts(source), source.indexOf('/*#query*/') + '/*#query*/'.length) }));
const provenanceFile = path.join(repository, 'vendor/typescript-7.0.2-lib/provenance.json');
const provenance = JSON.parse(fs.readFileSync(provenanceFile, 'utf8'));
const manifest = { schema: 1, checkpoint1: 'open', compiler: { version: semantic.version, upstream: provenance.upstream,
  nativeBinarySha256: digest(fs.readFileSync(semantic.binary)),
  nativePackageManifestSha256: digest(fs.readFileSync(path.join(path.dirname(semantic.binary), '../package.json'))),
  apiPackageManifestSha256: digest(fs.readFileSync(path.join(process.env.TYPE_SCRIPT_SEMANTIC_API, 'package.json'))),
  goModuleLocks: ['go.mod', 'go.sum'].map(file => { const bytes = fs.readFileSync(path.join(sources().go, file)); return { file, bytes: bytes.length, sha256: digest(bytes) }; }),
  libraryProvenance: record(provenanceFile) }, configurations, anchors, generated, inventory,
  setups: { inputReplay: 'Copy input-replay to a temporary project; move dual to node_modules/dual; prepend UTF-8 BOM to cases.mts and append the member controls exactly as tools/type-oracle.test.mjs does.' },
  expectations: 'Behavior-level pinned assertions in the inventoried oracle suites; complete design graph/counter/candidate coverage remains open.',
  corpusManifest: 'fixtures/types/command-center/input-manifest.json' };
const output = path.join(directory, 'oracle-manifest.json');
fs.writeFileSync(output, JSON.stringify(manifest, null, 2) + '\n');
process.stdout.write(`${relative(output)}: ${inventory.length} files, ${anchors.length} anchors, ${generated.length} generated inputs\n`);

import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import { completeCapture } from '../../../tools/type-corpus-capture.mjs';
const worktree = process.cwd();
const { compilerHookOracle } = await import(path.join(worktree, 'tools/type-oracle.mjs'));
const args = process.argv.slice(2);
if (args.length < 2 || args.length > 3 || (args[2] !== undefined && !/^--manifest=.+/.test(args[2])))
  throw Error('Usage: node fixtures/types/command-center/member-probe.mjs CORPUS OUTPUT [--manifest=PATH]');
const root = fs.realpathSync(path.resolve(args[0]));
const output = path.resolve(args[1]);
const manifestFile = args[2] === undefined ? path.join(worktree, 'fixtures/types/command-center/input-manifest.json')
  : path.resolve(args[2].slice('--manifest='.length));
const manifestBytes = fs.readFileSync(manifestFile);
const manifest = JSON.parse(manifestBytes);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
function verify() {
  if (manifest.physicalRoot !== undefined && manifest.physicalRoot !== root) throw Error('Corpus physical root changed');
  if (manifest.currentDirectory !== undefined && manifest.currentDirectory !== root) throw Error('Corpus working directory changed');
  const revision = execFileSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  if (revision !== manifest.revision) throw Error('Corpus revision changed');
  execFileSync('git', ['-C', root, 'diff', '--quiet', 'HEAD', '--']);
  for (const row of [...manifest.files, { file: manifest.configuration.file, sha256: manifest.configuration.sha256 }, ...manifest.locks]) {
    if (hash(fs.readFileSync(path.resolve(root, row.file))) !== row.sha256) throw Error('Corpus input changed: ' + row.file);
  }
}
const seeds = ['annotated-member', 'contextual-member', 'jsx-member'];
const selected = manifest.queries.filter(query => seeds.includes(query.id));
if (selected.length !== seeds.length || seeds.some(id => selected.filter(query => query.id === id).length !== 1))
  throw Error('Expected exactly one of each member probe seed: ' + seeds.join(', '));
verify();
let programBaseline;
if (manifest.schema === 2) {
  if (!Array.isArray(manifest.programFiles)) throw Error('Corpus manifest lacks ordered program inventory');
  const files = new Map(manifest.files.map(row => [row.file, row]));
  programBaseline = { compilerOptions: manifest.configuration.resolvedConfigOptions,
    files: manifest.programFiles.map(file => {
      const row = files.get(file);
      if (!row) throw Error('Ordered program input missing from manifest: ' + file);
      return { file: path.resolve(root, file), sha256: row.sha256,
        defaultLibrary: row.defaultLibrary, externalLibrary: row.externalLibrary };
    }) };
}
const out = { schema: 2, captureComplete: false, corpusRevision: manifest.revision, manifestSha256: hash(manifestBytes), history: 'baseline', diagnostics: 'none',
  ...(programBaseline === undefined ? {} : { programBaseline }),
  note: 'Exploratory compiler-only observations; fresh checker per demand, not production admission, normalized decoded Q3 groups, filter outcomes, or semantic performance. Before/after file checks do not provide transactional disk capture.',
  requests: selected.map(query => ({ id: query.id, request: { operation: 'member', at: query.at } })), observations: [] };
fs.writeFileSync(output, JSON.stringify(out, null, 2) + '\n');
for (const query of selected) {
  const start = performance.now();
  process.stdout.write('Starting ' + query.id + '\n');
  const answer = await compilerHookOracle(path.join(root, manifest.configuration.file), { operation: 'member', at: query.at });
  out.observations.push({ id: query.id, elapsedMilliseconds: performance.now() - start, request: { operation: 'member', at: query.at }, answer });
  fs.writeFileSync(output, JSON.stringify(out, null, 2) + '\n');
  process.stdout.write(JSON.stringify({ id: query.id, candidates: answer.candidateLedger.length, groups: answer.groups.length, occurrences: answer.groups.reduce((n, group) => n + group.occurrences.length, 0) }) + '\n');
}
verify();
completeCapture(output);
process.stdout.write('All input hashes still match\n');

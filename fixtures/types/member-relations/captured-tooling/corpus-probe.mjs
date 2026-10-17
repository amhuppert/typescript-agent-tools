import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import { fileURLToPath } from 'node:url';
const worktree = process.cwd();
const { compilerHookOracle, typeOracle } = await import(path.join(worktree, 'tools/type-oracle.mjs'));
const root = '/Users/alex/github/command-center';
const manifestFile = path.join(worktree, 'fixtures/types/command-center/input-manifest.json');
const manifestBytes = fs.readFileSync(manifestFile);
const manifest = JSON.parse(manifestBytes);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
function verify() {
  const revision = execFileSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  if (revision !== manifest.revision) throw Error('Corpus revision changed');
  execFileSync('git', ['-C', root, 'diff', '--quiet', 'HEAD', '--']);
  for (const row of [...manifest.files, { file: manifest.configuration.file, sha256: manifest.configuration.sha256 }, ...manifest.locks]) {
    if (hash(fs.readFileSync(path.resolve(root, row.file))) !== row.sha256) throw Error('Corpus input changed: ' + row.file);
  }
}
verify();
const selected = manifest.queries.filter(query => ['annotated-member', 'contextual-member', 'jsx-member'].includes(query.id));
const out = { schema: 1, corpusRevision: manifest.revision, manifestSha256: hash(manifestBytes), history: 'baseline', diagnostics: 'none',
  note: 'Exploratory compiler-only observations; fresh checker per demand, not production admission, normalized decoded Q3 groups, filter outcomes, or semantic performance. Before/after file checks do not provide transactional disk capture.', observations: [] };
for (const query of selected) {
  const start = performance.now();
  process.stdout.write('Starting ' + query.id + '\n');
  const answer = await compilerHookOracle(path.join(root, 'tsconfig.json'), { operation: 'member', at: query.at });
  out.observations.push({ id: query.id, elapsedMilliseconds: performance.now() - start, request: { operation: 'member', at: query.at }, answer });
  fs.writeFileSync(path.join(worktree, '.cc/temp/types-corpus-observations.json'), JSON.stringify(out, null, 2) + '\n');
  process.stdout.write(JSON.stringify({ id: query.id, candidates: answer.candidateLedger.length, groups: answer.groups.length, occurrences: answer.groups.reduce((n, group) => n + group.occurrences.length, 0) }) + '\n');
}
verify();
process.stdout.write('All input hashes still match\n');

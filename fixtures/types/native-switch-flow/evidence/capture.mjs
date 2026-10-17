import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';

const root = process.cwd();
const evidence = path.join(root, 'fixtures/types/native-switch-flow/evidence');
const apiRoot = '/Users/alex/github/typescript-agent-tools/.cache/compiler-oracles/typescript-7.0.2/package';
const binary = '/Users/alex/github/typescript-agent-tools/.cache/compiler-oracles/typescript-darwin-x64-7.0.2/package/lib/tsc';
const nativeSource = '/Users/alex/github/typescript-agent-tools/.cache/typescript-go-7.0.2';
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const row = file => {
  const bytes = fs.readFileSync(file);
  return { file: path.relative(root, file).split(path.sep).join('/'), bytes: bytes.length, sha256: digest(bytes) };
};
const live = [
  'tools/type-switch-flow-oracle.test.mjs', 'tools/type-oracle.mjs', 'tools/semantic-oracle.mjs',
  'tools/navigation-oracle.mjs', 'tools/corpus-selection.mjs', 'tools/vendor-typescript-libs.mjs',
  'fixtures/types/native-switch-flow/flow.ts', 'fixtures/types/native-switch-flow/tsconfig.json',
  'fixtures/types/native-switch-flow/loose.json', 'fixtures/types/native-switch-flow/native-observations.json',
];
fs.mkdirSync(path.join(evidence, 'captured-tooling'), { recursive: true });
const before = live.map(file => row(path.join(root, file)));
for (const file of live.filter(file => file.startsWith('tools/'))) {
  fs.copyFileSync(path.join(root, file), path.join(evidence, 'captured-tooling', path.basename(file)));
}
for (const file of ['cases.ts', 'strict.json', 'loose.json', 'probe.mjs', 'answers.json']) {
  fs.copyFileSync(path.join(root, '.cc/temp/types-switch-research', file), path.join(evidence, `research-${file}`));
}
const apiFiles = [];
const visit = directory => {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true }).sort((a,b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0)) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) visit(file);
    else if (entry.isFile()) {
      const bytes = fs.readFileSync(file);
      apiFiles.push({ file: path.relative(apiRoot, file).split(path.sep).join('/'), bytes: bytes.length, sha256: digest(bytes) });
    } else throw new Error(`Unsupported package inventory entry ${file}`);
  }
};
visit(apiRoot);
const json = (name, value) => fs.writeFileSync(path.join(evidence, name), JSON.stringify(value, null, 2) + '\n');
json('installed-api-inventory.json', apiFiles);
const git = args => {
  const result = spawnSync('git', ['-C', nativeSource, ...args], { encoding: 'utf8' });
  if (result.error || result.status !== 0) throw new Error(`Pinned source command failed: ${result.error?.message || result.stderr}`);
  return result.stdout.trim();
};
const pinnedCommit = git(['rev-parse', 'HEAD']);
if (pinnedCommit !== '2bd066d87f5bafd315be9f40889d0a60b9e58e0b' || git(['status', '--porcelain'])) throw new Error('Pinned source is not exact clean 7.0.2');
const started = new Date().toISOString();
const validation = spawnSync('cctl', ['validate', 'run', 'harness-test', '--queue-if-busy', '--require-match', '--', 'tools/type-switch-flow-oracle.test.mjs'], { encoding: 'utf8', timeout: 600000 });
const after = live.map(file => row(path.join(root, file)));
json('registered-replay.json', { started, finished: new Date().toISOString(), status: validation.status,
  signal: validation.signal, error: validation.error?.message, stdout: validation.stdout, stderr: validation.stderr });
if (JSON.stringify(before) !== JSON.stringify(after)) throw new Error('Replay tooling or fixtures changed while validation ran');
if (validation.error || validation.status !== 0 || validation.signal) throw new Error('Registered replay failed');
const result = {
  scope: 'fresh hash-frozen registered API replay; original scratch observations retain their separate weaker metadata provenance',
  compiler: '7.0.2', pinnedCommit, nativeSourceStatus: 'clean including untracked files',
  binary: row(binary), apiPackage: apiRoot, apiInventory: 'installed-api-inventory.json',
  requests: 86, diagnostics: 'freshRequest input config/program/syntax/bind gates only; no semantic diagnostics pass',
  inputHashes: 'API replay captures selected sourceHash, not compilerHookOracle full filesystem inputHashes; installed package inventory is separate',
  before, after,
  nativeSources: ['internal/binder/binder.go', 'internal/checker/flow.go'].map(file => {
    const result = row(path.join(nativeSource, file));
    result.file = file;
    return result;
  }),
};
json('replay-provenance.json', result);
process.stdout.write(validation.stdout);
process.stderr.write(validation.stderr);

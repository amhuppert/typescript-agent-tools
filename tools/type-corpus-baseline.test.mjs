import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const original = name => fileURLToPath(new URL(name, import.meta.url));

// Stub only the compiler transport: these subprocess tests pin capture CLI,
// snapshot verification and final artifact bytes, not TypeScript semantics.
function fixture(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'corpus-baseline-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const worktree = path.join(directory, 'tool');
  const root = path.join(directory, 'corpus');
  const scriptDir = path.join(worktree, 'fixtures/types/command-center');
  fs.mkdirSync(scriptDir, { recursive: true });
  fs.mkdirSync(path.join(worktree, 'tools'));
  fs.mkdirSync(root);
  for (const name of ['freeze.mjs', 'member-probe.mjs'])
    fs.copyFileSync(original('../fixtures/types/command-center/' + name), path.join(scriptDir, name));
  for (const name of ['type-corpus-capture.mjs', 'type-oracle-filesystem.mjs'])
    fs.copyFileSync(original('./'+name), path.join(worktree, 'tools', name));
  const texts = new Map([
    ['src/lib/specs/execution-service.ts', '\uFEFFinterface SpecWorkflowCleanupPort {\n  observe(): void;\n}\n'],
    ['src/lib/specs/workflow-cleanup-port.ts', 'const value = {\n    observe: () => {},\n};\n'],
    ['src/lib/agent-gateway/route-handlers.ts', 'export function createAgentGatewayHandlers() {}\n'],
    ['src/components/CollapsibleText.tsx', 'interface CollapsibleTextProps {\n  maxCollapsedHeight?: number;\n}\n'],
    ['src/lib/notepads/service.ts', 'function selected() {\n  return modes.filter(mode => mode);\n}\n'],
  ]);
  for (const [file, text] of texts) {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), text);
  }
  fs.writeFileSync(path.join(root, 'tsconfig.json'), '{}');
  fs.writeFileSync(path.join(root, 'package.json'), '{"private":true}');
  const binary = path.join(directory, 'compiler');
  fs.writeFileSync(binary, 'compiler identity');
  const apiPackage = path.join(directory, 'api');
  fs.mkdirSync(apiPackage);
  fs.writeFileSync(path.join(apiPackage, 'package.json'), '{"version":"7.0.2"}');
  fs.writeFileSync(path.join(worktree, 'tools/semantic-oracle.mjs'), `
    import fs from 'node:fs';
    import path from 'node:path';
    export { lineStarts, position } from ${JSON.stringify(new URL('./semantic-oracle.mjs', import.meta.url).href)};
    const names = ${JSON.stringify([...texts.keys()])};
    export async function loadSemanticApi() {
      return { version: '7.0.2', nativeVersion: 'Version 7.0.2', binary: ${JSON.stringify(binary)},
        API: class {
          constructor({ cwd }) { this.root = cwd; }
          updateSnapshot() {
            const root = this.root;
            return { dispose() {}, getProject() { return { program: {
              getConfigFileParsingDiagnostics() { return []; }, getProgramDiagnostics() { return []; },
              getSourceFileNames() { return names.map(name => path.join(root, name)); },
              getSourceFile(name) { return { text: process.env.CORPUS_TEST_WRONG_TEXT ? 'different' : fs.readFileSync(name, 'utf8').replace(/^\\uFEFF/, '') }; },
              getSourceFileMetadata() { return { isDefaultLibrary: false, isFromExternalLibrary: false }; },
              getCompilerOptions() { return {}; },
            } }; } };
          }
          close() {}
        }
      };
    }
  `);
  fs.writeFileSync(path.join(worktree, 'tools/navigation-oracle.mjs'),
    `export { compilerText } from ${JSON.stringify(new URL('./navigation-oracle.mjs', import.meta.url).href)};\n`);
  fs.writeFileSync(path.join(worktree, 'tools/type-oracle.mjs'), `
    import fs from 'node:fs'; import {createHash} from 'node:crypto';
    export async function compilerHookOracle(config) {
      return {status:'observed', candidateLedger:[], groups:[], inputFiles:[config],
        inputHashes:[{file:config,sha256:createHash('sha256').update(fs.readFileSync(config)).digest('hex')}]};
    }
  `);
  execFileSync('git', ['init', '-q', root]);
  execFileSync('git', ['-C', root, 'add', '.']);
  execFileSync('git', ['-C', root, '-c', 'user.name=Capture Test', '-c', 'user.email=capture@example.invalid', 'commit', '-q', '-m', 'fixture']);
  const revision = execFileSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  const output = path.join(directory, 'manifest.json');
  const run = (script, args, extraEnv = {}) => spawnSync(process.execPath, [path.join(scriptDir, script), root, ...args], {
    cwd: worktree, encoding: 'utf8', env: { ...process.env, TYPE_SCRIPT_SEMANTIC_API: apiPackage, ...extraEnv },
  });
  return { directory, root, texts, revision, output, run };
}

test('explicit corpus revision writes raw source/query hashes and preserves BOM identity', t => {
  const f = fixture(t);
  const result = f.run('freeze.mjs', [f.output, '--revision=' + f.revision]);
  assert.equal(result.status, 0, result.stderr);
  const manifest = JSON.parse(fs.readFileSync(f.output));
  assert.equal(manifest.revision, f.revision);
  assert.equal(manifest.physicalRoot, fs.realpathSync(f.root));
  assert.equal(manifest.currentDirectory, fs.realpathSync(f.root));
  assert.deepEqual(manifest.programFiles, [...f.texts.keys()]);
  assert.equal(manifest.programRediscovered, true);
  assert.equal(manifest.files.length, f.texts.size);
  for (const row of manifest.files) {
    const bytes = fs.readFileSync(path.join(f.root, row.file));
    assert.equal(row.sha256, hash(bytes));
    assert.equal(row.bytes, bytes.length);
  }
  for (const query of manifest.queries)
    assert.equal(query.sourceSha256, hash(fs.readFileSync(path.join(f.root, query.at.file))));
});

test('freeze rejects wrong/default revisions, dirty tracked files and changed compiler text', t => {
  const f = fixture(t);
  for (const args of [[f.output], [f.output, '--revision=' + '0'.repeat(40)]]) {
    const result = f.run('freeze.mjs', args);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /revision/);
    assert.equal(fs.existsSync(f.output), false);
  }
  const args = [f.output, '--revision=' + f.revision];
  const changed = f.run('freeze.mjs', args, { CORPUS_TEST_WRONG_TEXT: '1' });
  assert.notEqual(changed.status, 0);
  assert.match(changed.stderr, /source.*(differ|changed)/i);
  assert.equal(fs.existsSync(f.output), false);
  fs.appendFileSync(path.join(f.root, 'package.json'), '\n');
  const dirty = f.run('freeze.mjs', args);
  assert.notEqual(dirty.status, 0);
  assert.match(dirty.stderr, /tracked|git.*diff/);
  assert.equal(fs.existsSync(f.output), false);
});

test('member capture uses an explicit new manifest and finalizes its exact three requests', t => {
  const f = fixture(t);
  const queries = ['annotated-member', 'contextual-member', 'jsx-member'].map((id, index) => ({ id, at: 'cases.ts:1:' + (index + 1) }));
  const manifest = { revision: f.revision, files: [], locks: [], queries,
    configuration: { file: 'tsconfig.json', sha256: hash('{}') } };
  fs.writeFileSync(f.output, JSON.stringify(manifest));
  const before = fs.readFileSync(f.output);
  const capture = path.join(f.directory, 'members.json');
  const result = f.run('member-probe.mjs', [capture, '--manifest=' + f.output]);
  assert.equal(result.status, 0, result.stderr);
  const completed = JSON.parse(fs.readFileSync(capture));
  assert.equal(completed.captureComplete, true);
  assert.equal(completed.manifestSha256, hash(before));
  assert.equal(completed.corpusRevision, f.revision);
  assert.deepEqual(completed.requests, queries.map(query => ({ id: query.id, request: { operation: 'member', at: query.at } })));
  assert.equal(completed.observations.length, 3);
  assert.deepEqual(fs.readFileSync(f.output), before);
});


test('explicit corpus config selects and hashes a controlled native execution profile',t=>{
  const f=fixture(t);
  const text='{"extends":"./tsconfig.json","compilerOptions":{"singleThreaded":true}}';
  fs.writeFileSync(path.join(f.root,'tsconfig.native-corpus.json'),text);
  const result=f.run('freeze.mjs',[f.output,'--revision='+f.revision,'--config=tsconfig.native-corpus.json']);
  assert.equal(result.status,0,result.stderr);
  const manifest=JSON.parse(fs.readFileSync(f.output));
  assert.equal(manifest.configuration.file,'tsconfig.native-corpus.json');
  assert.equal(manifest.configuration.sha256,hash(text));
  assert.equal(fs.readFileSync(path.join(f.root,'tsconfig.json'),'utf8'),'{}',
    'selecting a new baseline config must preserve the original tracked config');
});

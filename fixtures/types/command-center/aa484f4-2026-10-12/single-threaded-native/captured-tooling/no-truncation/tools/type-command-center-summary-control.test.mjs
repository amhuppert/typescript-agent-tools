// Temporary registered research driver; archived after execution rather than
// adding an external corpus prerequisite to the default harness suite.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import { test } from 'node:test';
import { compilerHookOracle } from './type-oracle.mjs';
import { completeCapture } from './type-corpus-capture.mjs';

const evidence = path.resolve('fixtures/types/command-center/aa484f4-2026-10-12/single-threaded-native');
const manifestFile = path.join(evidence, 'input-manifest.json');
const manifestBytes = fs.readFileSync(manifestFile);
const manifest = JSON.parse(manifestBytes);
const root = manifest.physicalRoot;
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const files = new Map(manifest.files.map(row => [row.file, row]));
const programBaseline = { compilerOptions: manifest.configuration.resolvedConfigOptions,
  files: manifest.programFiles.map(file => {
    const row = files.get(file);
    assert.ok(row, 'ordered program source must be in manifest');
    return { file: path.resolve(root, file), sha256: row.sha256,
      defaultLibrary: row.defaultLibrary, externalLibrary: row.externalLibrary };
  }) };
const toolkit = ['tools/type-oracle.mjs', 'tools/semantic-oracle.mjs',
  'tools/navigation-oracle.mjs', 'tools/vendor-typescript-libs.mjs',
  'tools/corpus-selection.mjs', 'tools/type-corpus-capture.mjs', 'tools/type-oracle-filesystem.mjs',
  'tools/type-command-center-summary-control.test.mjs',
  'tools/type-oracle-compiler.mjs',
  'fixtures/types/compiler-hook/checker-hook.go',
  'fixtures/types/compiler-hook/ls-hook.go', 'fixtures/types/compiler-hook/oracle-test.go'];
const toolkitHashes = toolkit.map(file => ({ file, sha256: hash(fs.readFileSync(file)) }));
function verify() {
  assert.equal(manifest.configuration.resolvedConfigOptions.singleThreaded,true);
  assert.equal(fs.realpathSync(root), root);
  assert.equal(execFileSync('git', ['-C', root, 'rev-parse', 'HEAD'], {encoding:'utf8'}).trim(), manifest.revision);
  execFileSync('git', ['-C', root, 'diff', '--quiet', 'HEAD', '--']);
  assert.equal(hash(fs.readFileSync(manifestFile)), hash(manifestBytes));
  for (const row of [...manifest.files, manifest.configuration, ...manifest.locks])
    assert.equal(hash(fs.readFileSync(path.resolve(root, row.file))), row.sha256, row.file);
  for (const row of toolkitHashes) assert.equal(hash(fs.readFileSync(row.file)), row.sha256, row.file);
}
async function capture(kind) {
  verify();
  const members = kind === 'members';
  const selected = manifest.queries.filter(query => query.id === 'predicate-callback');
  assert.equal(selected.length, 1);
  const requests = selected.map(query => ({id:query.id,
    request:{operation:'type',at:query.at,question:query.question,noTruncation:true,inspectSummaryBytes:true}}));
  const outputFile = path.join(evidence, 'no-truncation-observations.json');
  assert.equal(fs.existsSync(outputFile),false,'completed or interrupted prior captures must not be overwritten');
  const document = {schema:2,captureComplete:false,corpusRevision:manifest.revision,
    manifestSha256:hash(manifestBytes),history:'baseline',diagnostics:'none',programBaseline,
    note:'Fresh native checker observations on explicit singleThreaded inputs; capture timings are not product semantic latency or filter acceptance.',
    requests,observations:[]};
  fs.writeFileSync(outputFile,JSON.stringify(document,null,2)+'\n');
  document.toolkitHashes = toolkitHashes;
  for (const row of requests.slice(document.observations.length)) {
    console.log('Starting '+kind+'/'+row.id);
    const start = performance.now();
    const answer = await compilerHookOracle(path.join(root,manifest.configuration.file),row.request);
    assert.equal(answer.errorType, false);
    assert.equal(Buffer.from(answer.summaryBytes, 'base64').toString('utf8'), answer.summary);
    assert.equal(Buffer.byteLength(answer.summary), 431);
    assert.ok(Array.isArray(answer.nativeProgramFiles)&&answer.nativeProgramFiles.length===manifest.files.length);
    document.observations.push({id:row.id,elapsedMilliseconds:performance.now()-start,request:row.request,answer});
    fs.writeFileSync(outputFile,JSON.stringify(document,null,2)+'\n');
    console.log(JSON.stringify({id:row.id,status:answer.status,summary:answer.summary,
      groups:answer.groups?.length,candidates:answer.candidateLedger?.length}));
  }
  verify();
  const completed = completeCapture(outputFile);
  assert.equal(completed.captureComplete,true);
  assert.equal(completed.observations.length,requests.length);
  assert.ok(completed.inputHashes.length>=manifest.files.length);
  console.log('Verified complete capture '+outputFile);
}
const kind = 'types';
test('native NoTruncation corpus summary capture',()=>capture(kind));

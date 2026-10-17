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

const evidence = path.resolve('fixtures/types/command-center/aa484f4-2026-10-12');
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
  'tools/corpus-selection.mjs', 'tools/type-corpus-capture.mjs',
  'tools/type-command-center-capture.test.mjs',
  'fixtures/types/compiler-hook/checker-hook.go',
  'fixtures/types/compiler-hook/ls-hook.go', 'fixtures/types/compiler-hook/oracle-test.go'];
const toolkitHashes = toolkit.map(file => ({ file, sha256: hash(fs.readFileSync(file)) }));
function verify() {
  assert.equal(fs.realpathSync(root), root);
  assert.equal(execFileSync('git', ['-C', root, 'rev-parse', 'HEAD'], {encoding:'utf8'}).trim(), manifest.revision);
  execFileSync('git', ['-C', root, 'diff', '--quiet', 'HEAD', '--']);
  assert.equal(hash(fs.readFileSync(manifestFile)), hash(manifestBytes));
  for (const row of [...manifest.files, manifest.configuration, ...manifest.locks])
    assert.equal(hash(fs.readFileSync(path.resolve(root, row.file))), row.sha256, row.file);
  for (const row of toolkitHashes) assert.equal(hash(fs.readFileSync(row.file)), row.sha256, row.file);
}
function verifyPrefix(document, requests) {
  assert.equal(document.captureComplete, false);
  assert.equal(document.manifestSha256, hash(manifestBytes));
  assert.equal(document.corpusRevision, manifest.revision);
  assert.deepEqual(document.requests, requests);
  assert.deepEqual(document.programBaseline, programBaseline);
  assert.ok(document.observations.length > 0 && document.observations.length < requests.length);
  document.observations.forEach((row, index) => {
    assert.equal(row.id, requests[index].id);
    assert.deepEqual(row.request, requests[index].request);
  });
  // The established finalizer verifies every recovered answer against the full
  // program baseline and all consumed metadata before the next native demand.
  const prefixFile = path.resolve('.cc/temp/stage66-corpus-resume-prefix.json');
  const prefix = {...document, requests: requests.slice(0, document.observations.length)};
  fs.writeFileSync(prefixFile, JSON.stringify(prefix));
  try { completeCapture(prefixFile); } finally { fs.unlinkSync(prefixFile); }
}
async function capture(kind) {
  verify();
  const members = kind === 'members';
  const selected = manifest.queries.filter(query => !members ||
    ['annotated-member', 'contextual-member', 'jsx-member'].includes(query.id));
  assert.equal(selected.length, members ? 3 : 5);
  const requests = selected.map(query => ({id:query.id,
    request:{operation:members?'member':'type',at:query.at,...(members?{}:{question:query.question})}}));
  const outputFile = path.join(evidence, members?'member-observations.json':'type-observations.json');
  let document;
  if (fs.existsSync(outputFile)) {
    document = JSON.parse(fs.readFileSync(outputFile));
    verifyPrefix(document, requests);
    const interrupted = path.join(evidence, 'interrupted-capture');
    fs.mkdirSync(interrupted, {recursive:true});
    const archived = path.join(interrupted, path.basename(outputFile));
    assert.equal(fs.existsSync(archived), false, 'do not overwrite prior interrupted evidence');
    fs.copyFileSync(outputFile, archived);
  } else {
    document = {schema:2,captureComplete:false,corpusRevision:manifest.revision,
      manifestSha256:hash(manifestBytes),history:'baseline',diagnostics:'none',programBaseline,
      note:'Fresh native checker observations on isolated inputs; native capture timings are not product semantic latency or filter acceptance.',
      requests,observations:[]};
    fs.writeFileSync(outputFile,JSON.stringify(document,null,2)+'\n');
  }
  document.toolkitHashes = toolkitHashes;
  for (const row of requests.slice(document.observations.length)) {
    console.log('Starting '+kind+'/'+row.id);
    const start = performance.now();
    const answer = await compilerHookOracle(path.join(root,manifest.configuration.file),row.request);
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
const kind = 'members';
test('native isolated corpus '+kind+' capture',()=>capture(kind));

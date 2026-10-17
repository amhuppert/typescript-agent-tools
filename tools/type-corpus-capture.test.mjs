import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { createHash } from 'node:crypto';
import { test } from 'node:test';
import { execFileSync, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { completeCapture } from './type-corpus-capture.mjs';

function fixture(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'type-corpus-capture-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const entries = [['cases.ts', '\uFEFFexport const value = 1;\n'], ['package.json', '{"type":"module"}'], ['base.json', '{"compilerOptions":{"strict":true}}']];
  const rows = entries.map(([name, text]) => {
    const file = path.join(directory, name);
    fs.writeFileSync(file, text);
    return { file, sha256: createHash('sha256').update(fs.readFileSync(file)).digest('hex') };
  }).sort((a, b) => a.file < b.file ? -1 : a.file > b.file ? 1 : 0);
  const file = path.join(directory, 'capture.json');
  const request = { operation: 'member', at: 'cases.ts:1:14' };
  const document = { schema: 2, captureComplete: false, requests: [{ id: 'first', request }], observations: [{ id: 'first', request, answer: { status: 'observed', inputFiles: rows.map(row => row.file), inputHashes: rows.map(row => ({ ...row })) } }] };
  const save = () => fs.writeFileSync(file, JSON.stringify(document));
  save();
  return { directory, rows, file, document, save };
}

test('completed artifact verifies metadata and raw BOM bytes for every consumed input', t => {
  const f = fixture(t);
  const completed = completeCapture(f.file);
  assert.equal(completed.captureComplete, true);
  assert.deepEqual(JSON.parse(fs.readFileSync(f.file, 'utf8')), completed);
  assert.deepEqual(completed.inputHashes, f.rows);
});

test('a legacy path-only answer cannot be finalized as a verified capture', t => {
  const f = fixture(t);
  delete f.document.observations[0].answer.inputHashes;
  f.save();
  const before = fs.readFileSync(f.file);
  assert.throws(() => completeCapture(f.file), /input hashes/);
  assert.deepEqual(fs.readFileSync(f.file), before);
});

test('metadata mutation after demand remains a failed unfinished artifact', t => {
  const f = fixture(t);
  fs.writeFileSync(path.join(f.directory, 'package.json'), '{"type":"commonjs"}');
  const before = fs.readFileSync(f.file);
  assert.throws(() => completeCapture(f.file), /input changed/);
  assert.deepEqual(fs.readFileSync(f.file), before);
});

test('missing hash entries and inconsistent cross-request replay inputs fail', t => {
  const f = fixture(t);
  f.document.observations[0].answer.inputHashes.pop();
  f.save();
  assert.throws(() => completeCapture(f.file), /input hashes/);
  f.document.observations[0].answer.inputHashes = f.rows;
  const request = { operation: 'member', at: 'cases.ts:1:15' };
  f.document.requests.push({ id: 'second', request });
  f.document.observations.push({ id: 'second', request, answer: { status: 'observed', inputFiles: f.rows.map(row => row.file), inputHashes: f.rows.map((row, index) => ({ ...row, sha256: index === 0 ? '0'.repeat(64) : row.sha256 })) } });
  f.save();
  assert.throws(() => completeCapture(f.file), /inconsistent replay input/);
  assert.equal(JSON.parse(fs.readFileSync(f.file, 'utf8')).captureComplete, false);
});

test('malformed schema, lifecycle, answer status and duplicate input identities remain unfinished', t => {
  for (const [change, message] of [
    [document => { document.schema = 1; }, /schema/],
    [document => { delete document.captureComplete; }, /unfinished/],
    [document => { document.observations[0].answer.status = 'failed'; }, /observed/],
    [document => { delete document.observations[0].answer.status; }, /observed/],
    [document => {
      const answer = document.observations[0].answer;
      answer.inputFiles.push(answer.inputFiles[0]);
      answer.inputHashes.push({ ...answer.inputHashes[0] });
    }, /duplicate input/],
  ]) {
    const f = fixture(t);
    change(f.document);
    f.save();
    const before = fs.readFileSync(f.file);
    assert.throws(() => completeCapture(f.file), message);
    assert.deepEqual(fs.readFileSync(f.file), before);
  }
});

test('expected requests reject omissions, duplicates, unexpected IDs and changed demands', t => {
  for (const change of [
    document => { delete document.requests; },
    document => { document.requests = []; },
    document => { document.requests.push({ ...document.requests[0] }); },
    document => { document.requests.push({ id: 'missing', request: { operation: 'member', at: 'cases.ts:1:15' } }); },
    document => { document.observations.push({ ...document.observations[0] }); },
    document => { document.observations[0].id = 'unexpected'; },
    document => { document.observations[0].request = { operation: 'type', at: 'cases.ts:1:14' }; },
  ]) {
    const f = fixture(t);
    change(f.document);
    f.save();
    const before = fs.readFileSync(f.file);
    assert.throws(() => completeCapture(f.file), /request/);
    assert.deepEqual(fs.readFileSync(f.file), before);
  }
});

test('a completed historical artifact cannot be finalized again', t => {
  const f = fixture(t);
  completeCapture(f.file);
  const before = fs.readFileSync(f.file);
  assert.throws(() => completeCapture(f.file), /unfinished/);
  fs.writeFileSync(path.join(f.directory, 'package.json'), '{}');
  assert.throws(() => completeCapture(f.file), /unfinished/);
  assert.deepEqual(fs.readFileSync(f.file), before);
});

test('program baseline join accepts extra hashed metadata and rejects changed order, provenance, content or options', t => {
  for (const change of [
    undefined,
    answer => answer.programFiles.reverse(),
    answer => { answer.programFiles[0].externalLibrary = true; },
    answer => { answer.programFiles[0].defaultLibrary = true; },
    (answer, document) => { document.programBaseline.files[0].sha256 = '0'.repeat(64); },
    answer => { answer.compilerOptions.strict = false; },
    answer => { delete answer.programFiles; },
  ]) {
    const f = fixture(t);
    // Two source inputs and a third consumed metadata file: the join must
    // require every expected source without discarding other captured reads.
    f.document.programBaseline = {
      files: f.rows.slice(0, 2).map(row => ({ ...row, defaultLibrary: false, externalLibrary: false })),
      compilerOptions: { strict: true },
    };
    const answer = f.document.observations[0].answer;
    answer.programFiles = f.document.programBaseline.files.map(({ sha256, ...row }) => ({ ...row }));
    answer.compilerOptions = { strict: true };
    change?.(answer, f.document);
    f.save();
    if (change) {
      const before = fs.readFileSync(f.file);
      assert.throws(() => completeCapture(f.file), /program|source/i);
      assert.deepEqual(fs.readFileSync(f.file), before);
    } else {
      const completed = completeCapture(f.file);
      assert.equal(completed.captureComplete, true);
      assert.deepEqual(completed.inputHashes, f.rows);
    }
  }
});

test('member capture validates all three seeds and starts a fresh unfinished artifact before demand', t => {
  const f = fixture(t);
  const worktree = path.join(f.directory, 'worktree');
  const root = path.join(f.directory, 'corpus');
  const manifestFile = path.join(worktree, 'fixtures/types/command-center/input-manifest.json');
  fs.mkdirSync(path.dirname(manifestFile), { recursive: true });
  fs.mkdirSync(path.join(worktree, 'tools'), { recursive: true });
  fs.mkdirSync(root);
  fs.writeFileSync(path.join(worktree, 'tools/type-oracle.mjs'), 'export async function compilerHookOracle() { throw Error("first demand failed"); }\n');
  fs.writeFileSync(path.join(root, 'tsconfig.json'), '{}');
  execFileSync('git', ['init', '-q', root]);
  execFileSync('git', ['-C', root, '-c', 'user.name=Capture Test', '-c', 'user.email=capture@example.invalid', 'commit', '-q', '--allow-empty', '-m', 'fixture']);
  const revision = execFileSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  const queries = ['annotated-member', 'contextual-member', 'jsx-member'].map((id, index) => ({ id, at: 'cases.ts:1:' + (index + 1) }));
  const manifest = { revision, files: [], locks: [], configuration: { file: 'tsconfig.json', sha256: createHash('sha256').update('{}').digest('hex') }, queries };
  const probe = fileURLToPath(new URL('../fixtures/types/command-center/member-probe.mjs', import.meta.url));
  const old = '{"captureComplete":true,"old":true}\n';
  for (const selected of [[], queries.slice(0, 1), queries.slice(0, 2), [queries[0], queries[1], queries[1]]]) {
    fs.writeFileSync(manifestFile, JSON.stringify({ ...manifest, queries: selected }));
    fs.writeFileSync(f.file, old);
    const result = spawnSync(process.execPath, [probe, root, f.file], { cwd: worktree, encoding: 'utf8' });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /exactly one.*member probe/);
    assert.equal(fs.readFileSync(f.file, 'utf8'), old);
  }
  fs.writeFileSync(manifestFile, JSON.stringify(manifest));
  fs.writeFileSync(f.file, old);
  const result = spawnSync(process.execPath, [probe, root, f.file], { cwd: worktree, encoding: 'utf8' });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /first demand failed/);
  const unfinished = JSON.parse(fs.readFileSync(f.file, 'utf8'));
  assert.equal(unfinished.captureComplete, false);
  assert.deepEqual(unfinished.observations, []);
  assert.deepEqual(unfinished.requests, queries.map(query => ({ id: query.id, request: { operation: 'member', at: query.at } })));
});

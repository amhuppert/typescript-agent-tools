// Temporary registered research driver. Its exact source is archived with the
// baseline after execution; this external 1.52 GB corpus is not a default test.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { test } from 'node:test';

const evidence = path.resolve('fixtures/types/command-center/aa484f4-2026-10-12');
const provenance = JSON.parse(fs.readFileSync(path.join(evidence, 'snapshot-provenance.json')));
test('freeze the explicit isolated Command Center baseline', () => {
  const manifestFile = path.join(evidence, 'input-manifest.json');
  assert.equal(fs.existsSync(manifestFile), false, 'a completed historical manifest must not be overwritten');
  const output = execFileSync(process.execPath, ['fixtures/types/command-center/freeze.mjs',
    provenance.snapshotRoot, manifestFile, '--revision=' + provenance.revision], {
    encoding: 'utf8', maxBuffer: 1 << 24,
  });
  const manifest = JSON.parse(fs.readFileSync(manifestFile));
  assert.equal(manifest.schema, 2);
  assert.equal(manifest.revision, provenance.revision);
  assert.equal(manifest.physicalRoot, provenance.snapshotRoot);
  assert.equal(manifest.currentDirectory, provenance.snapshotRoot);
  assert.equal(manifest.programFiles.length, manifest.files.length);
  assert.equal(manifest.programRediscovered, true);
  assert.deepEqual(manifest.queries.map(query => query.id),
    ['annotated-member', 'contextual-member', 'inferred-factory', 'jsx-member', 'predicate-callback']);
  console.log(output.trim());
});

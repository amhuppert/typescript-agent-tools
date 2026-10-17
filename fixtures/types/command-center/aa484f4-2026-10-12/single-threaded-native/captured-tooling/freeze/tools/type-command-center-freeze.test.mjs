// Temporary registered corpus driver; archive then remove after terminal.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
import {test} from 'node:test';
const base=path.resolve('fixtures/types/command-center/aa484f4-2026-10-12');
const provenance=JSON.parse(fs.readFileSync(path.join(base,'single-threaded-native/profile-provenance.json')));
test('freeze explicit single-threaded native corpus configuration',()=>{
  const output=path.join(base,'single-threaded-native/input-manifest.json');
  assert.equal(fs.existsSync(output),false);
  const result=execFileSync(process.execPath,['fixtures/types/command-center/freeze.mjs',
    provenance.physicalRoot,output,'--revision='+provenance.revision,'--config='+provenance.configuration.file],
    {encoding:'utf8',maxBuffer:1<<25});
  const manifest=JSON.parse(fs.readFileSync(output));
  assert.equal(manifest.configuration.resolvedConfigOptions.singleThreaded,true);
  assert.equal(manifest.configuration.sha256,provenance.configuration.sha256);
  assert.equal(manifest.programRediscovered,true);
  assert.equal(manifest.files.length,7639);
  assert.equal(manifest.queries.length,5);
  console.log(result.trim());
});

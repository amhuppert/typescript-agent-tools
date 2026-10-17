import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

// Replay an isolated frozen fixture in its current physical directory. Only
// captured input-path metadata changes roots; semantic identities, ranges,
// source bytes, diagnostics, hashes and paths outside the fixture stay exact.
export function assertFixtureCapture(actual, expected, root) {
  const currentRoot=fs.realpathSync(root).split(path.sep).join('/');
  const relocated=expected.map(row=>{
    const copy=structuredClone(row),answer=copy.answer;
    const oldRoot=path.posix.dirname(answer.compilerOptions.configFilePath);
    const relocate=file=>file.startsWith(oldRoot+'/')?currentRoot+file.slice(oldRoot.length):file;
    answer.compilerOptions.configFilePath=relocate(answer.compilerOptions.configFilePath);
    for(const field of ['apiProgramFiles','nativeProgramFiles','programFiles','inputHashes']) {
      for(const entry of answer[field]) entry.file=relocate(entry.file);
    }
    answer.inputFiles=answer.inputFiles.map(relocate);
    return copy;
  });
  assert.deepEqual(actual,relocated);
}

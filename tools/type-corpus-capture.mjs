// Finalize a compiler capture only after checking every consumed replay input.
// This does not make before/after disk verification transactional capture.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { isDeepStrictEqual } from 'node:util';

export function completeCapture(file) {
  const document = JSON.parse(fs.readFileSync(file, 'utf8'));
  if (document.schema !== 2) throw Error('Capture schema must be 2');
  if (document.captureComplete !== false) throw Error('Only an unfinished capture can be finalized');
  if (!Array.isArray(document.requests) || !document.requests.length)
    throw Error('A capture must list its expected requests');
  const requests = new Map();
  for (const row of document.requests) {
    if (typeof row?.id !== 'string' || !row.id.length || !row.request
        || typeof row.request !== 'object' || Array.isArray(row.request) || requests.has(row.id))
      throw Error('Expected requests must have unique IDs and exact demands');
    requests.set(row.id, row.request);
  }
  if (!Array.isArray(document.observations) || !document.observations.length)
    throw Error('A capture must contain requested observations');
  if (document.observations.length !== requests.size)
    throw Error('Capture observations do not match all expected requests');
  const baseline = document.programBaseline;
  let expectedProgram;
  if (baseline !== undefined) {
    if (!Array.isArray(baseline?.files) || !baseline.files.length || !baseline.compilerOptions
        || typeof baseline.compilerOptions !== 'object' || Array.isArray(baseline.compilerOptions))
      throw Error('Program baseline requires ordered source identities and compiler options');
    const seen = new Set();
    expectedProgram = baseline.files.map(row => {
      if (typeof row?.file !== 'string' || !path.isAbsolute(row.file) || seen.has(row.file)
          || !/^[0-9a-f]{64}$/.test(row.sha256) || typeof row.defaultLibrary !== 'boolean'
          || typeof row.externalLibrary !== 'boolean')
        throw Error('Program baseline source identity is invalid or duplicated');
      seen.add(row.file);
      return { file: row.file, defaultLibrary: row.defaultLibrary, externalLibrary: row.externalLibrary };
    });
  }
  const inputs = new Map();
  const observed = new Set();
  for (const { id, request, answer } of document.observations) {
    if (!requests.has(id) || observed.has(id) || !isDeepStrictEqual(request, requests.get(id)))
      throw Error('Capture observation does not match a unique expected request');
    observed.add(id);
    if (answer?.status !== 'observed') throw Error('Every requested answer must be observed');
    if (!Array.isArray(answer?.inputFiles) || !Array.isArray(answer.inputHashes)
        || !answer.inputFiles.length || answer.inputFiles.length !== answer.inputHashes.length)
      throw Error('Consumed input hashes are missing or incomplete');
    const requestInputs = new Map();
    for (const [index, row] of answer.inputHashes.entries()) {
      if (typeof row?.file !== 'string' || row.file !== answer.inputFiles[index] || !path.isAbsolute(row.file)
          || !/^[0-9a-f]{64}$/.test(row.sha256))
        throw Error('Consumed input hashes do not match exact disk input identities');
      if (requestInputs.has(row.file)) throw Error('Capture contains a duplicate input identity');
      requestInputs.set(row.file, row.sha256);
      const previous = inputs.get(row.file);
      if (previous !== undefined && previous !== row.sha256)
        throw Error('Capture has an inconsistent replay input: ' + row.file);
      inputs.set(row.file, row.sha256);
    }
    if (baseline !== undefined) {
      if (!isDeepStrictEqual(answer.programFiles, expectedProgram))
        throw Error('Captured program source order or provenance differs from baseline');
      if (!isDeepStrictEqual(answer.compilerOptions, baseline.compilerOptions))
        throw Error('Captured program compiler options differ from baseline');
      for (const row of baseline.files)
        if (requestInputs.get(row.file) !== row.sha256)
          throw Error('Captured program source bytes differ from baseline: ' + row.file);
    }
  }
  verifyCapturedInputs(inputs);
  document.inputHashes = [...inputs].map(([file, sha256]) => ({ file, sha256 }))
    .sort((a, b) => a.file < b.file ? -1 : a.file > b.file ? 1 : 0);
  document.captureComplete = true;
  fs.writeFileSync(file, JSON.stringify(document, null, 2) + '\n');
  return document;
}

/** Recheck the exact bytes a native capture consumed. Both capture publication
 * and product replay use this operation, including metadata and library files.
 * Bracketing a run is change detection, not a transactional filesystem snapshot.
 */
export function verifyCapturedInputs(inputs) {
  for (const [input, expected] of inputs) {
    const actual = createHash('sha256').update(fs.readFileSync(input)).digest('hex');
    if (actual !== expected) throw Error('Consumed input changed: ' + input);
  }
}

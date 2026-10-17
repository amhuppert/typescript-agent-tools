// Diagnostic-only mutant comparison. Mutant outputs are never native goldens.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {compilerHookOracle as detachedOracle} from '../.cc/temp/merge-publication-counterfactual-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const rows = [];
for (const [shape, directory, names] of [
  ['self', '.cc/temp/native-merge-publication-evidence', ['both', 'directA', 'directB']],
  ['mixed', '.cc/temp/native-mixed-merge-publication-evidence', ['bothDirect']],
]) {
  test(`diagnostic detached table counterfactual: ${shape}`, async () => {
    const root = path.resolve(directory);
    const source = fs.readFileSync(path.join(root, 'consumer.ts'), 'utf8');
    for (const profile of ['strict', 'nocheck']) for (const name of names) {
      const marker = `/*#${name}*/`;
      const at = {file:'consumer.ts', ...position(lineStarts(source), source.indexOf(marker) + marker.length)};
      const config = path.join(root, `${profile}.json`);
      const request = {at, history:'cold', inspectDiagnosticCache:true};
      const native = await compilerHookOracle(config, request);
      const mutant = await detachedOracle(config, request);
      for (const value of [native, mutant]) {
        assert.equal(value.compiler, '7.0.2');
        assert.equal(value.checkerCount, 1);
      }
      rows.push({shape, profile, name, at,
        native: {summary:native.summary, errorType:native.errorType, diagnostics:native.beforeSummary.diagnosticCodes, failures:native.beforeSummary.demandFailures},
        mutant: {mutation:'mergeSymbolTable delayed table writes only; not compiler goldens', summary:mutant.summary, errorType:mutant.errorType, diagnostics:mutant.beforeSummary.diagnosticCodes, failures:mutant.beforeSummary.demandFailures},
        inputHashes:native.inputHashes});
      assert.deepEqual(native.inputHashes, mutant.inputHashes);
      fs.writeFileSync('.cc/temp/stage66-merge-publication-counterfactual.json', JSON.stringify(rows,null,2)+'\n');
    }
  });
}

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-relation-recovery/', import.meta.url));
// Each source preserves the maintained 99/100/101-depth fixture exactly,
// adding only a same-level property before or after its existing next chain.
// Missing is on Target: native isPropertySymbolTypeRelated reads Target then
// admits target Any before acquiring Source, including during the subtype pass.
for (const profile of ['strict', 'loose', 'exact']) {
  test(`native ordered property recovery and relation overflow (${profile})`, async () => {
    const rows = [];
    for (const depth of [99, 100, 101]) {
      for (const order of ['before', 'after', 'healthy']) {
        const name = `${order}${depth}`;
        const source = fs.readFileSync(path.join(directory, `${name}.ts`), 'utf8');
        const at = {file: `${name}.ts`, ...position(lineStarts(source), source.indexOf('selected ='))};
        const config = path.join(directory, `${name}.${profile}.json`);
        const api = await typeOracle(config, {at, properties: false});
        const native = await compilerHookOracle(config, {at});
        rows.push({name, depth, order, compiler: native.compiler,
          summary: native.summary, errorType: native.errorType,
          publicSummary: api.summary, publicErrorType: api.errorType,
          diagnostics: api.diagnostics,
          codes: native.beforeSummary.diagnosticCodes,
          demandFailures: native.beforeSummary.demandFailures,
          sourceDepth: native.beforeSummary.counters.relationSourceDepth,
          targetDepth: native.beforeSummary.counters.relationTargetDepth,
          first: native.beforeSummary.observations.relationFirstFinish});
      }
    }
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-relation-recovery-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    for (const row of rows) {
      const overflow = row.depth >= 100;
      const missing = row.order === 'before' || row.order === 'after' && !overflow;
      assert.equal(row.compiler, '7.0.2', row.name);
      assert.equal(row.summary, overflow ? '"same"' : '"chosen"', row.name);
      assert.equal(row.publicSummary, row.summary, row.name);
      assert.equal(row.errorType, false, row.name);
      assert.equal(row.publicErrorType, false, row.name);
      assert.deepEqual(row.diagnostics, [], row.name);
      assert.deepEqual(row.demandFailures, overflow ? ['relation_depth_limit'] : [], row.name);
      // Diagnostic codes are source-sorted presence evidence, not an acquisition
      // trace. Native propertiesRelatedTo order plus the depth counters establish
      // that the before property is consumed before the limit; after is skipped.
      assert.deepEqual(row.codes.filter(code => code === 2304 || code === 2321),
        [...(missing ? [2304] : []), ...(overflow ? [2321] : [])], row.name);
      assert.equal(row.sourceDepth, Math.min(row.depth, 100), row.name);
      assert.equal(row.targetDepth, Math.min(row.depth, 100), row.name);
      assert.equal(row.first.overflow, overflow, row.name);
      assert.equal(row.first.result, overflow ? 0 : -1, row.name);
    }
  });
}

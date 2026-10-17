import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-combined-alias/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const marker = '/*#preferredValue*/';
const at = {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(marker) + marker.length)};

// This declaration-name demand has no enclosing call prelude. Keep both
// histories explicit: an earlier demand must not explain the export candidate.
for (const profile of ['strict', 'loose']) {
  test(`combined alias preferred value is independent of normalization (${profile})`, async () => {
    const rows = [];
    for (const history of ['cold', 'baseline']) {
      const config = path.join(root, `${profile}.json`);
      const native = await compilerHookOracle(config, {at, history});
      const api = await typeOracle(config, {at, history, properties: false});
      rows.push({history, native, api});
    }
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-combined-alias-history-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    for (const {history, native, api} of rows) {
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], history);
      assert.equal(native.summary, 'typeof Preferred', history);
      assert.equal(native.errorType, false, history);
      assert.deepEqual(native.beforeSummary.demandFailures, [], history);
      assert.deepEqual(api.prelude, [], history);
    }
  });
}

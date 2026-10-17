import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-assertion-completion/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const config of ['tsconfig', 'loose', 'exact', 'erasable']) {
  test(`native assertion and satisfies continuation (${config})`, async () => {
    const rows = [];
    for (const match of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      if (config === 'erasable' && match[1] !== 'angleHealthy') continue;
      const at = {file: 'cases.ts', ...position(lineStarts(source), match.index + match[0].length)};
      const native = await compilerHookOracle(path.join(root, `${config}.json`), {at});
      const api = await typeOracle(path.join(root, `${config}.json`), {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], match[1]);
      rows.push({name: match[1], kind: api.kind, summary: api.summary, errorType: api.errorType,
        codes: native.beforeSummary.diagnosticCodes,
        demandFailures: native.beforeSummary.demandFailures,
        symbolError: native.beforeSummary.observations.nonMissingSymbolErrorType ?? null});
    }
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-assertion-completion-${config}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.equal(rows.length, config === 'erasable' ? 1 : 11);
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${config}.json`), 'utf8')));
  });
}

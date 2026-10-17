import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-contextual-patterns/', import.meta.url));
// Variance is held constant to isolate assignParameterType's unknown-pattern
// fallback. Strict-null and exact optionality remain independently varied.
for (const [config, file, count] of [
  ['tsconfig', 'cases.ts', 27], ['loose', 'cases.ts', 27], ['exact', 'cases.ts', 27],
  ['tsconfig', 'target.ts', 4],
]) {
  test(`native contextual binding pattern assignment (${config}/${file})`, async () => {
    const source = fs.readFileSync(path.join(root, file), 'utf8');
    const rows = [];
    for (const match of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file, ...position(lineStarts(source), match.index + match[0].length)};
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
    const label = `${config}-${file.slice(0, -3)}`;
    fs.writeFileSync(`.cc/temp/native-contextual-patterns-${label}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.equal(rows.length, count);
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${label}.json`), 'utf8')));
  });
}

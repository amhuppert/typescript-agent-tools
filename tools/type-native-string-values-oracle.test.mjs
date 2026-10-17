import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-string-values/', import.meta.url));
const main = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const [name, newline] of [['LF', '\n'], ['CR', '\r'], ['CRLF', '\r\n'], ['LS', '\u2028'], ['PS', '\u2029']]) {
  assert.ok(main.includes(`/*#continued${name}*/"\\uD800\\${newline}\\uDC00"`), `${name} source bytes`);
}

for (const group of [{prefix:'', file:'cases.ts', count:34},
  {prefix:'diagnostics-', file:'diagnostics.ts', count:4},
  {prefix:'diagnostics-no-check-', file:'diagnostics.ts', count:4}]) {
  const source = fs.readFileSync(path.join(root, group.file), 'utf8');
  for (const mode of ['strict', 'loose']) {
    const profile = group.prefix + mode;
    test(`native exact source strings (${profile})`, async () => {
      const config = path.join(root, `${profile}.json`);
      const rows = [];
      for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
        const name = marker[1];
        const at = {file: group.file, ...position(lineStarts(source), marker.index + marker[0].length)};
        const native = await compilerHookOracle(config, {at, inspectDiagnosticCache:true});
        const api = await typeOracle(config, {at, properties:false});
        assert.equal(native.compiler, '7.0.2');
        assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], name);
        assert.equal(native.errorType, false, name);
        assert.deepEqual(native.beforeSummary.demandFailures, [], name);
        // The reached call prelude still compares enum values under noCheck.
        const diagnostic = group.prefix !== '' && name !== 'enumEqual';
        assert.deepEqual(native.beforeSummary.diagnosticCodes, diagnostic ? [2345] : [], name);
        if (name.endsWith('Equality')) assert.equal(native.summary, 'false', name);
        if (['fixedFixed','fixedBraced','bracedFixed','bracedBraced','scalar','typeTemplate','continuedTypeTemplate','enumConcatTemplate'].includes(name)) {
          assert.equal(native.summary, '"𐀀"', name);
        }
        rows.push({name, summary:native.summary, errorType:native.errorType,
          location:api.location, origins:api.origins,
          codes:native.beforeSummary.diagnosticCodes, demandFailures:native.beforeSummary.demandFailures,
          acquisitions:native.beforeSummary.displayAcquisitions ?? []});
      }
      assert.equal(rows.length, group.count);
      fs.mkdirSync('.cc/temp', {recursive:true});
      fs.writeFileSync(`.cc/temp/native-string-values-${profile}.json`, JSON.stringify(rows,null,2) + '\n');
      assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
    });
  }
}

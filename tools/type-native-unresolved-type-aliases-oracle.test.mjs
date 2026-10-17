import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-unresolved-type-aliases/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const config of ['tsconfig', 'loose', 'exact']) {
  test(`native unresolved type aliases retain ordered arguments (${config})`, async () => {
    const rows = [];
    for (const match of source.matchAll(/\/\*#([^*]+)\*\//g)) {
      const at = {file: 'cases.ts', ...position(lineStarts(source), match.index + match[0].length)};
      const native = await compilerHookOracle(path.join(root, `${config}.json`), {at});
      const api = await typeOracle(path.join(root, `${config}.json`), {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], match[1]);
      rows.push({name: match[1], summary: native.summary, errorType: native.errorType,
        codes: native.beforeSummary.diagnosticCodes,
        demandFailures: native.beforeSummary.demandFailures,
        defaultError: native.beforeSummary.observations.typeParameterDefaultError ?? null});
    }
    assert.equal(rows.length, 27);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-unresolved-type-aliases-${config}.json`, JSON.stringify(rows, null, 2) + '\n');
    const expected = path.join(root, `expected-${config}.json`);
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(expected, 'utf8')));
  });
}


test('native existing namespace failure is located on the missing right identifier', async () => {
  const tag = '/*#existingNamespace*/';
  for (const profile of ['tsconfig', 'loose', 'exact']) {
    const answer = await typeOracle(path.join(root, `${profile}.json`), {
      at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length)},
      diagnostics: true,
    });
    const diagnostic = answer.diagnostics.find(row => row.code === 2694 && row.message.includes('Available'));
    assert.ok(diagnostic);
    const offset = source.indexOf('/*@existingNamespace*/') + '/*@existingNamespace*/Available.'.length;
    assert.deepEqual(diagnostic.location, {file: 'cases.ts', start: position(lineStarts(source), offset), end: position(lineStarts(source), offset + 'Missing'.length)});
  }
});

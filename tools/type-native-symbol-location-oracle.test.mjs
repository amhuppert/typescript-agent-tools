import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-symbol-location/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
function at(marker, sigil = '@') {
  const tag = `/*${sigil}${marker}*/`, offset = source.indexOf(tag);
  assert.notEqual(offset, -1, marker);
  return {file: 'cases.ts', ...position(lineStarts(source), offset + tag.length)};
}
for (const profile of ['tsconfig', 'loose', 'exact']) {
  test(`native symbol location and actual variable reads (${profile})`, async () => {
    const config = path.join(root, `${profile}.json`), rows = [];
    for (const file of ['cases.ts', 'merge-read.ts']) {
    const text = fs.readFileSync(path.join(root, file), 'utf8');
    for (const marker of text.matchAll(/\/\*#([^*]+)\*\//g)) {
      const selected = {file, ...position(lineStarts(text), marker.index + marker[0].length)};
      const native = await compilerHookOracle(config, {at: selected});
      const api = await typeOracle(config, {at: selected, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
      rows.push({file, name: marker[1], kind: api.kind, summary: api.summary, errorType: api.errorType,
        codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures});
    }
    }
    const members = [];
    for (const marker of ['field', 'bindingProperty', 'privateDeclaration', 'privateIn']) {
      const answer = await compilerHookOracle(config, {operation: 'member', at: at(marker)});
      members.push({name: marker, groups: answer.nativeRangeGroups, rawSymbolFlags: answer.rawSymbolFlags,
        candidates: answer.candidateObservations, demandFailures: answer.demandFailures});
    }
    const output = {rows, members};
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-symbol-location-${profile}.json`, JSON.stringify(output, null, 2) + '\n');
    assert.deepEqual(output, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

// Native `import("...")` type nodes: getTypeFromImportTypeNode answers,
// their diagnostics, and member references through import-type qualifiers.
// The Rust test replays expected.json; this test proves it is native 7.0.2.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {memberOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fs.realpathSync(fileURLToPath(new URL('../fixtures/types/native-import-types/', import.meta.url)));
const config = path.join(root, 'tsconfig.json');
// Module displays name absolute paths; rows keep them relative to the fixture.
const portable = text => text.split(root).join('<root>');

function markers(file) {
  const source = fs.readFileSync(path.join(root, file), 'utf8');
  return [...source.matchAll(/\/\*#([^*]+)\*\//g)].map(marker =>
    ({name: marker[1], file, at: {file, ...position(lineStarts(source), marker.index + marker[0].length)}}));
}

const at = location => ({file: location.file, line: location.start.line, column: location.start.column});

test('import type nodes, their failures and qualifier references match TypeScript 7.0.2', async () => {
  const types = [];
  let diagnostics;
  for (const marker of ['src/fetch.ts', 'src/cases.ts', 'src/checks.ts', 'src/aliases.ts', 'src/errors.ts'].flatMap(markers)) {
    try {
      const answer = await typeOracle(config, {at: marker.at, properties: false, diagnostics: diagnostics === undefined});
      assert.equal(answer.compiler, '7.0.2');
      diagnostics ??= answer.diagnostics.map(diagnostic =>
        ({code: diagnostic.code, ...at(diagnostic.location)}));
      types.push({name: marker.name, file: marker.file, kind: answer.kind,
        summary: portable(answer.summary), errorType: answer.errorType});
    } catch (error) {
      if (!/^UnsupportedTarget:/.test(error.message)) throw error;
      types.push({name: marker.name, file: marker.file, unsupported: true});
    }
  }
  const references = [];
  for (const marker of markers('src/lib.ts')) {
    const answer = await memberOracle(config, {at: marker.at});
    assert.equal(answer.compiler, '7.0.2');
    references.push({name: marker.name, file: marker.file, groups: answer.rawReferenceGroups.map(group => ({
      origins: group.origins.map(origin => at(origin.location)),
      occurrences: group.occurrences.map(at),
    }))});
  }
  const rows = {compiler: '7.0.2', types, diagnostics, references};
  const expected = path.join(root, 'expected.json');
  if (!fs.existsSync(expected)) {
    const captured = path.join(os.tmpdir(), 'native-import-types-expected.json');
    fs.writeFileSync(captured, JSON.stringify(rows, null, 2) + '\n');
    throw new Error(`Captured native rows in ${captured}; review and install them as ${expected}`);
  }
  assert.deepEqual(rows, JSON.parse(fs.readFileSync(expected, 'utf8')));
  assert.equal(types.length, 49);
  assert.equal(references.length, 4);
});

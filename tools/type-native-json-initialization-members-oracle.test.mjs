import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const fixture = fileURLToPath(new URL('../fixtures/types/native-json-initialization/', import.meta.url));
const selected = ['own-direct', 'own-star', 'nullable', 'nested-array', 'whole-object', 'whole-string'];
for (const profile of ['strict', 'loose']) {
  test(`native JSON initialization preserves member reference identity (${profile})`, async () => {
    const rows = [];
    const cases = JSON.parse(fs.readFileSync(path.join(fixture, 'cases.json'), 'utf8'));
    for (const entry of cases.filter(entry => selected.includes(entry.name))) {
      const root = path.resolve('.cc/temp/native-json-initialization-members', profile, entry.name);
      fs.mkdirSync(root, {recursive: true});
      for (const [file, text] of Object.entries(entry.files)) fs.writeFileSync(path.join(root, file), text);
      const config = path.join(root, 'tsconfig.json');
      fs.copyFileSync(path.join(fixture, `${profile}.json`), config);
      const source = entry.files['main.ts'];
      const marker = '/*#detail*/';
      const at = {file: 'main.ts', ...position(lineStarts(source), source.indexOf(marker) + marker.length)};
      const native = await compilerHookOracle(config, {operation: 'member', at});
      assert.equal(native.compiler, '7.0.2');
      assert.equal(native.checkerCount, 1);
      assert.deepEqual(native.demandFailures, [], entry.name);
      rows.push({name: entry.name, groups: native.nativeRangeGroups,
        flags: native.rawSymbolFlags, origins: native.rawOrigins});
    }
    fs.writeFileSync(`.cc/temp/native-json-initialization-members-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.equal(rows.length, selected.length);
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(fixture, `expected-members-${profile}.json`), 'utf8')));
  });
}

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const fixture = fileURLToPath(new URL('../fixtures/types/native-json-initialization/', import.meta.url));
for (const profile of ['strict', 'loose']) {
  test(`native augmented JSON value properties retain their exact origins (${profile})`, async () => {
    const rows = [];
    const cases = JSON.parse(fs.readFileSync(path.join(fixture, 'cases.json'), 'utf8'));
    for (const entry of cases) {
      const root = path.resolve('.cc/temp/native-json-initialization-properties', profile, entry.name);
      fs.mkdirSync(root, {recursive: true});
      for (const [file, text] of Object.entries(entry.files)) fs.writeFileSync(path.join(root, file), text);
      const config = path.join(root, 'tsconfig.json');
      fs.copyFileSync(path.join(fixture, `${profile}.json`), config);
      const source = entry.files['main.ts'];
      const marker = '/*#alias*/';
      const at = {file: 'main.ts', ...position(lineStarts(source), source.indexOf(marker) + marker.length)};
      const native = await typeOracle(config, {at, history: 'cold', properties: true});
      assert.equal(native.compiler, '7.0.2');
      rows.push({name: entry.name, summary: native.summary, errorType: native.errorType,
        properties: native.properties.map(property => ({name: property.name,
          origins: property.origins.map(origin => origin.location)}))});
    }
    fs.writeFileSync(`.cc/temp/native-json-initialization-properties-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.equal(rows.length, 12);
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(fixture, `expected-properties-${profile}.json`), 'utf8')));
  });
}

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-apparent/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const markers = JSON.parse(fs.readFileSync(path.join(root, 'markers.json'), 'utf8'));

for (const config of ['tsconfig.json', 'loose.json', 'unchecked.json']) {
  test(`native apparent types acquire actual primitive and constraint members (${config})`, async () => {
    for (const marker of markers) {
      const tag = `/*#${marker}*/`;
      const answer = await typeOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length)},
        properties: false,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.kind, 'expression', marker);
      assert.equal(answer.summary, 'string | number', marker);
      assert.equal(answer.errorType, false, marker);
      assert.deepEqual(answer.prelude, [], marker);
    }
  });
}

test('native selected reads preserve the actual destructured-property recovery dependency', async () => {
  for (const config of ['tsconfig.json', 'missing.json', 'arity.json', 'kind.json']) {
    for (const marker of markers.filter(marker => marker !== 'constrained')) {
      const tag = `/*#${marker}*/`;
      const answer = await compilerHookOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length)},
        operation: 'type',
      });
      assert.equal(answer.summary, 'string | number', `${config}/${marker}`);
      assert.deepEqual(answer.beforeSummary.observations.destructuredPropertyReturn,
        {errorType: config !== 'tsconfig.json'}, `${config}/${marker}: actual consumed assignment dependency`);
    }
  }
});

for (const config of ['missing.json', 'arity.json', 'kind.json']) {
  test(`native missing or malformed primitive wrappers consume global recovery (${config})`, async () => {
    for (const marker of markers.filter(marker => marker !== 'constrained')) {
      const tag = `/*#${marker}*/`;
      const answer = await typeOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length)},
        properties: false,
      });
      assert.equal(answer.kind, 'expression', marker);
      assert.equal(answer.summary, 'string | number', marker);
      // getGlobalType recovers as emptyObjectType and destructured-property
      // lookup returns errorType. Flow preserves the declared union despite
      // that consumed dependency; the final summary alone cannot prove validity.
    }
  });
}

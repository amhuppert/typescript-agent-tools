import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-property-lookups/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const names = JSON.parse(fs.readFileSync(path.join(root, 'names.json'), 'utf8'));

test('native binary64 literals establish canonical shortest-decimal ties and thresholds', async () => {
  const literals = JSON.parse(fs.readFileSync(path.join(root, 'literals.json'), 'utf8'));
  const source = fs.readFileSync(path.join(root, 'literals.ts'), 'utf8');
  for (const [index, {text}] of literals.entries()) {
    const at = {file: 'literals.ts', ...position(lineStarts(source), source.indexOf(`numeric${index} =`))};
    // The pinned public API cannot marshal a nonfinite literal's numeric
    // value. The native hook reads the same checker answer before JSON, and
    // transfers its exact display text without coercing that payload.
    const answer = text === 'Infinity'
      ? await compilerHookOracle(path.join(root, 'tsconfig.json'), {at, operation: 'type'})
      : await typeOracle(path.join(root, 'tsconfig.json'), {at, properties: false});
    assert.equal(answer.kind, 'value');
    assert.equal(answer.summary, text);
    assert.equal(answer.errorType, false);
    if (text !== 'Infinity') assert.deepEqual(answer.prelude, []);
  }
});

for (const config of ['tsconfig.json', 'loose.json', 'unchecked.json']) {
  test(`native numeric-name applicability through actual assignment reads (${config})`, async () => {
    for (const {marker, name, numeric} of names) {
      const tag = `/*#${marker}*/`;
      const answer = await typeOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length)},
        properties: false,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.kind, 'expression', JSON.stringify(name));
      assert.equal(answer.summary, numeric && config !== 'unchecked.json' ? 'number' : 'string | number', JSON.stringify(name));
      assert.equal(answer.errorType, false, JSON.stringify(name));
      assert.deepEqual(answer.prelude, []);
    }
  });
}

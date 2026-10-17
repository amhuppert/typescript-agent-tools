import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/qualified-access/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const config of ['tsconfig.json', 'loose.json', 'unchecked.json']) {
  test(`native qualified access retains each segment and actual flow (${config})`, async () => {
    const optional = config === 'loose.json' ? 'string' : 'string | undefined';
    for (const [name, kind, summary, errorType = false] of [
      ['whole', 'type', 'number'], ['root', 'value', 'Holder'],
      ['middle', 'value', 'Inner'], ['leaf', 'value', 'number'],
      ['fixed', 'type', '42'], ['optional', 'type', optional],
      ['indexed', 'type', config === 'unchecked.json' ? 'number | undefined' : 'number'],
      ['any', 'type', 'any'], ['missing', 'type', 'any', true],
      ['nullable', 'type', '42'], ['unrelated', 'type', optional],
      ['assigned', 'type', 'string'], ['loop', 'type', optional],
    ]) {
      const marker = `/*#${name}*/`;
      const answer = await typeOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(marker) + marker.length)}, properties: false,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.kind, kind, name);
      assert.equal(answer.summary, summary, name);
      assert.equal(answer.errorType, errorType, name);
    }
  });
}

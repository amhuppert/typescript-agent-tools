// Recursive function serialization is an actual-symbol question, not alias metadata.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/function-symbols/', import.meta.url));
const observations = [
  ['functions.ts', 'top', '() => typeof top'],
  ['functions.ts', 'privateTop', '() => typeof privateTop'],
  ['functions.ts', 'local', '() => ...'],
  ['functions.ts', 'member', '() => typeof member'],
  ['functions.ts', 'arrow', '() => typeof arrow'],
  ['functions.ts', 'escaped', '() => typeof \\u0052aw'],
  ['consumer.ts', 'renamed', '() => typeof top'],
  ['consumer.ts', 'importedEscaped', '() => typeof \\u0052aw'],
];

test('native visited function types preserve nonlocal symbols and source-written names', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [file, name, summary] of observations) {
      const source = fs.readFileSync(path.join(directory, file), 'utf8');
      const marker = `/*#${name}*/`;
      assert.equal(source.split(marker).length, 2);
      const answer = await typeOracle(path.join(directory, config), {
        at: { file, ...position(lineStarts(source), source.indexOf(marker) + marker.length) },
        properties: false,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.history, 'baseline');
      assert.equal(answer.kind, 'value');
      assert.equal(answer.summary, summary, `${config}: ${name}`);
      assert.equal(answer.errorType, false, `${config}: ${name}`);
      assert.deepEqual(answer.prelude, []);
    }
  }
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-access/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const fixed = {
  dot: 'number', literal: 'number', template: 'number', escaped: 'number',
  empty: 'bigint', decoded: 'boolean', identifier: 'number', union: 'number',
  fixed: '42', chainKnown: '42', chainAny: 'any', anyDot: 'any', anyElement: 'any',
};
for (const config of ['tsconfig.json', 'loose.json', 'unchecked.json', 'exact.json', 'dot-index.json']) {
  test(`native property and element reads preserve key/option decisions (${config})`, async () => {
    const optional = config === 'loose.json' ? '' : ' | undefined';
    const unchecked = config === 'unchecked.json' ? ' | undefined' : '';
    const answers = {...fixed, optional: `string${optional}`, optionalElement: `string${optional}`,
      chain: `number${optional}`, elementChain: `number${optional}`,
      stringIndex: `string | number${unchecked}`, stringElement: `string | number${unchecked}`,
      numberIndex: `number${unchecked}`, numericName: `number${unchecked}`,
      nonnumericName: `string | number${unchecked}`, specialNumericName: `number${unchecked}`};
    for (const [name, summary] of Object.entries(answers)) {
      const answer = await typeOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(`const ${name} =`) + 6)},
        properties: false,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.kind, 'value', name);
      assert.equal(answer.summary, summary, name);
      assert.equal(answer.errorType, false, name);
      assert.deepEqual(answer.prelude, [], name);
    }
  });
}

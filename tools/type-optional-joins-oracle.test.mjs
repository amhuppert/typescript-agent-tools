import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/optional-joins/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const config of ['tsconfig.json', 'loose.json']) {
  test(`native optionality and truthiness joins restore whole read types (${config})`, async () => {
    const optional = config === 'loose.json' ? '' : ' | undefined';
    for (const [name, summary] of [
      ...['afterDot', 'afterElement', 'afterKeyEffect', 'afterReceiverEffect', 'falsyArm', 'afterBranch'].map(name => [name, `Box${optional}`]),
      ['afterCall', `Handler${optional}`], ['narrowedArm', 'Box'],
      ['afterNullish', `string${optional}`], ['afterTruthy', `string${optional}`], ['truthyArm', 'string'],
    ]) {
      const answer = await typeOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(`const ${name} =`) + 6)}, properties: false,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.kind, 'value', name);
      assert.equal(answer.summary, summary, name);
      assert.equal(answer.errorType, false, name);
    }
  });
}

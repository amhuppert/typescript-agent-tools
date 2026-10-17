import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = path.resolve('fixtures/types/native-property-inference');
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const expected = {inherited: 'number', optional: 'number | undefined', missing: 'any',
  staticSameName: 'number', noBase: 'any', staticNoBase: 'any', emptyConstructor: 'any',
  assignedConstructor: 'number', emptyStatic: 'any', assignedStatic: 'number', instanceWithStaticBlock: 'number', staticWithConstructor: 'number',
  declareWithConstructor: 'number', staticDeclareWithBlock: 'number'};
test('native unannotated property inference separates base lookup and automatic flow', async () => {
  const rows = [];
  for (const config of ['strict.json', 'implicit-any.json', 'loose.json', 'nocheck.json', 'missing-globals.json']) {
    for (const match of source.matchAll(/\/\*#(\w+)\*\//g)) {
      const marker = match[1];
      const answer = await compilerHookOracle(path.join(root, config), {
        history: 'cold',
        at: {file: 'cases.ts', ...position(lineStarts(source), match.index + match[0].length)},
      });
      rows.push({config, marker, answer});
      fs.mkdirSync('.cc/temp', {recursive: true});
      fs.writeFileSync('.cc/temp/native-property-inference.json', JSON.stringify(rows, null, 2)+'\n');
      assert.equal(answer.checkerCount, 1);
      assert.equal(answer.errorType, false, `${config}/${marker}`);
      assert.deepEqual(answer.demandFailures, [], `${config}/${marker}`);
      const summary = config === 'implicit-any.json' ? 'any'
        : config === 'loose.json' && marker === 'optional' ? 'number' : expected[marker];
      assert.equal(answer.summary, summary, `${config}/${marker}`);
    }
  }
});

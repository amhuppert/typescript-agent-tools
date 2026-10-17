import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = path.resolve('fixtures/types/native-constructor-selection');
test('bodyless constructors do not infer unannotated properties from flow', async () => {
  const rows = [];
  for (const config of ['strict.json', 'implicit-any.json', 'nocheck.json']) {
    for (const file of ['cases.ts', 'declarations.d.ts']) {
      const source = fs.readFileSync(path.join(root, file), 'utf8');
      for (const match of source.matchAll(/\/\*#(\w+)\*\//g)) {
        const marker = match[1];
        const answer = await compilerHookOracle(path.join(root, config), {
          at: {file, ...position(lineStarts(source), match.index + match[0].length)},
        });
        assert.equal(answer.errorType, false, `${config}/${marker}`);
        assert.equal(answer.summary, marker === 'implementedRead' ? 'number' : 'any', `${config}/${marker}`);
        assert.deepEqual(answer.demandFailures, [], `${config}/${marker}`);
        rows.push({config, file, marker, answer});
      }
    }
  }
  fs.mkdirSync('.cc/temp', {recursive: true});
  fs.writeFileSync('.cc/temp/native-constructor-selection.json', JSON.stringify(rows, null, 2)+'\n');
});

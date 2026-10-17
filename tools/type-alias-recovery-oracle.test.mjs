import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-generic-aliases/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');

test('native alias defaults retain acquisition evidence even when substitution discards the argument', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [marker, summary, acquired] of [
      ['ignoredDefault', 'number', true],
      ['ignoredSupplied', 'number', false],
      ['dependent', 'Defaulted<"value", "value">', false],
    ]) {
      const tag = `/*#${marker}*/`;
      const answer = await compilerHookOracle(path.join(root, config), {
        at: {file:'cases.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length)},
        operation:'type',
      });
      assert.equal(answer.summary, summary, `${config}/${marker}`);
      const observations = answer.beforeSummary.observations;
      assert.deepEqual(observations.typeParameterDefaultError, acquired ? {parameter:'T',hasSymbol:true} : undefined,
        `${config}/${marker}: actual default acquisition`);
      assert.equal(observations.typeParameterMappedError, undefined,
        `${config}/${marker}: no recovery target substituted into a demanded value`);
    }
  }
});

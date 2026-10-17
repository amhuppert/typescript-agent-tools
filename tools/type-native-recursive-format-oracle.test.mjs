import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
const root = path.resolve('fixtures/types/native-recursive-format');
test('recursive formatted aliases retain a healthy shallow member result', async () => {
  for (const config of ['tsconfig.json', 'loose.json', 'exact.json']) {
    for (const [file, markers] of [
      ['cases.ts', ['object', 'nested', 'array', 'tuple', 'any', 'unknown', 'primitive', 'generic', 'relation', 'relationGeneric', 'contextual']],
      ['variants.ts', ['ObjectOnly', 'NoFlatten', 'Nonrecursive']],
    ]) {
      const source = fs.readFileSync(path.join(root, file), 'utf8');
      for (const marker of markers) {
        const tag = `/*#${marker}*/`;
        const label = `${config}/${file}/${marker}`;
        const answer = await compilerHookOracle(path.join(root, config), {at: {file, ...position(lineStarts(source), source.indexOf(tag) + tag.length)}});
        assert.equal(answer.errorType, false, label);
        assert.equal(answer.summary, marker.startsWith('relation') ? 'number' : marker === 'contextual' || file === 'variants.ts' ? 'string' : 'string[]', label);
        assert.deepEqual(answer.demandFailures, [], label);
        assert.ok(!answer.beforeSummary.diagnosticCodes.includes(2589), label);
      }
    }
  }
});

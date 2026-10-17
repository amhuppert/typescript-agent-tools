import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, loadSemanticApi, position} from './semantic-oracle.mjs';

const fixtures = fileURLToPath(new URL('../fixtures/types/', import.meta.url));
const {SyntaxKind} = await loadSemanticApi();

// Native getWidenedProperty reads each installed property Type slot. A later
// selected property read is another acquisition, even when both use its cache.
// Product recovery evidence preserves those reads rather than deduplicating it.
for (const strict of [0, 1]) {
  test(`native JSON recovery keeps complete construction and property replay (${strict})`, async () => {
    const rows = [];
    for (const [name, directory, markers] of [
      ['missing', 'json-array-targets/missing', ['healthy', 'numbers']],
      ['wrong-arity', 'json-array-targets-review/augmentation-before-script', ['healthy', 'numbers']],
      ['rich', 'json-native-basic', ['stringValue']],
    ]) {
      const root = path.resolve('.cc/temp/json-recovery-replay', String(strict), name);
      fs.mkdirSync(root, {recursive: true});
      for (const entry of fs.readdirSync(path.join(fixtures, directory), {withFileTypes: true})) {
        if (entry.isFile()) fs.copyFileSync(path.join(fixtures, directory, entry.name), path.join(root, entry.name));
      }
      if (name === 'rich') fs.copyFileSync(path.join(fixtures, 'json-native/data.json'), path.join(root, 'data.json'));
      const config = path.join(root, `strict-${strict}-exact-0-unchecked-0.json`);
      const source = fs.readFileSync(path.join(root, 'consumer.ts'), 'utf8');
      for (const marker of markers) {
        const tag = `/*#${marker}*/`;
        assert.ok(source.includes(tag));
        const at = {file: 'consumer.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length)};
        const native = await compilerHookOracle(config, {at, history: 'cold',
          inspectDiagnosticCache: true, inspectExpressionAcquisitions: true});
        const api = await typeOracle(config, {at, history: 'cold', properties: false});
        assert.equal(native.compiler, '7.0.2');
        assert.equal(native.checkerCount, 1);
        assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType]);
        assert.deepEqual([native.summary, native.errorType], [marker === 'numbers' ? '{}' : 'string', false]);
        assert.deepEqual(native.beforeSummary.demandFailures, []);
        const acquisitions = native.beforeSummary.displayAcquisitions;
        const arrays = acquisitions.filter(event => event.event === 'exit'
          && event.operation === 'checkExpressionEx' && event.kind === SyntaxKind.ArrayLiteralExpression);
        assert.equal(arrays.length, name === 'rich' ? 7 : 1);
        assert.ok(arrays.every(event => !event.errorType));
        const numbers = acquisitions.filter(event => event.event === 'exit'
          && event.operation === 'getTypeOfSymbol' && event.name === 'numbers');
        assert.equal(numbers.length, marker === 'numbers' ? 2 : 1,
          `${name}/${marker}: widened-property and selected-property reads`);
        if (name === 'rich') {
          for (const key of ['objectAugmentation:a', 'objectAugmentation:b']) {
            assert.equal(native.beforeSummary.observations[key], undefined, key);
          }
          for (const property of ['empty', 'numbers', 'mixed', 'shapes', 'recursive']) {
            assert.equal(acquisitions.filter(event => event.event === 'exit'
              && event.operation === 'getTypeOfSymbol' && event.name === property).length, 1,
            `${name}/${property}: one widened-property read`);
          }
        }
        rows.push({name, marker, summary: native.summary, errorType: native.errorType,
          arrays, numbers, observations: native.beforeSummary.observations,
          acquisitions, inputHashes: native.inputHashes});
      }
    }
    fs.writeFileSync(`.cc/temp/native-json-recovery-replay-${strict}.json`, JSON.stringify(rows, null, 2) + '\n');
  });
}

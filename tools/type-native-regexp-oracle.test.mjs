import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-regexp/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
test('native regular expressions read the actual RegExp global and its members', async () => {
  const rows = [];
  for (const profile of ['valid', 'missing', 'arity', 'kind']) {
    for (const marker of ['literal', 'escaped', 'read']) {
      const tag = `/*#${marker}*/`;
      const at = {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(tag) + tag.length)};
      const native = await compilerHookOracle(path.join(root, profile + '.json'), {at});
      const api = await typeOracle(path.join(root, profile + '.json'), {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType]);
      assert.equal(native.summary, profile === 'valid' ? marker === 'read' ? 'string' : 'RegExp' : marker === 'read' ? 'any' : '{}');
      assert.equal(native.errorType, profile !== 'valid' && marker === 'read');
      rows.push({profile, marker, native});
    }
  }
  fs.mkdirSync('.cc/temp', {recursive: true});
  fs.writeFileSync('.cc/temp/native-regexp-observations.json', JSON.stringify(rows, null, 2));
});

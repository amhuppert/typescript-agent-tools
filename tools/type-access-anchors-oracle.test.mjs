import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/access-anchors/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const config of ['tsconfig.json', 'loose.json', 'exact.json']) {
  test(`native access anchors preserve read/write, diagnostic and flow questions (${config})`, async () => {
    for (const [name, summary, errorType] of [
      ['read', 'number', false], ['readAccessor', 'string', false],
      ['writeAccessor', 'boolean', false], ['compoundAccessor', 'string', false],
      ['writeOptional', config === 'tsconfig.json' ? 'string | undefined' : 'string', false],
      ['readonlyProperty', 'any', true], ['readonlyIndex', 'number', false],
      ['missing', 'any', true], ['element', 'number', false], ['key', '"value"', false],
      ['call', 'number', false],
    ]) {
      const marker = `/*#${name}*/`;
      const answer = await typeOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(marker) + marker.length)}, properties: false,
      });
      assert.equal(answer.compiler, '7.0.2');
      assert.equal(answer.kind, 'expression', name);
      assert.equal(answer.summary, summary, name);
      assert.equal(answer.errorType, errorType, name);
    }
    for (const [name, summary] of [
      ['unrelatedRead', config === 'loose.json' ? 'string' : 'string | undefined'],
      ['assignedRead', 'string'],
    ]) {
      const answer = await typeOracle(path.join(root, config), {
        at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(`const ${name}`) + 6)}, properties: false,
      });
      assert.equal(answer.summary, summary, name);
      assert.equal(answer.errorType, false, name);
    }
  });
}

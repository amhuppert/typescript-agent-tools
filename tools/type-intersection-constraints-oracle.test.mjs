import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { lineStarts, position } from './semantic-oracle.mjs';
import { typeOracle } from './type-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-intersection-constraints/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const sourceHash = createHash('sha256').update(source).digest('hex');
const starts = lineStarts(source);

test('intersection constraint substitution preserves changed, absent and unchanged constituents', async t => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [marker, expected] of [
      ['ordinary', '{ p: string; } & { q: number; }'],
      ['changed', '({ p: string; } | { p: number; }) & { q: boolean; }'],
      ['absent', '{ p: string; } | { p: number; }'],
      ['nested', '({ q: string; } | { q: number; }) & { p: boolean; }'],
      ['nullable', config === 'tsconfig.json' ? '{ p: string; } & { q: number; }' : 'T & { q: number; }'],
      ['unchanged', 'T & U'],
      ['invalid', 'any'],
    ]) {
      await t.test(`${config}/${marker}`, async () => {
        const tag = `/*#${marker}*/`;
        const offset = source.indexOf(tag) + tag.length;
        const answer = await typeOracle(path.join(root, config), {
          at: { file: 'cases.ts', ...position(starts, offset) }, properties: false, diagnostics: false,
        });
        assert.equal(answer.compiler, '7.0.2');
        assert.equal(answer.history, 'baseline');
        assert.equal(answer.kind, 'expression');
        assert.equal(answer.sourceHash, sourceHash);
        assert.equal(answer.summary, expected, `${config}/${marker}`);
        assert.equal(answer.errorType, marker === 'invalid');
        assert.deepEqual(answer.prelude, []);
        assert.deepEqual(answer.diagnostics, []);
      });
    }
  }
});

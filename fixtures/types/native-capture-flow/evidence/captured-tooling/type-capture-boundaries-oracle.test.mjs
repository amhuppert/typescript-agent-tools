import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-capture-boundaries/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'boundaries.ts'), 'utf8');
const changed = new Set(['variable-after', 'expression-after', 'do-after', 'switch-after', 'try-after', 'class-after']);
const scalar = new Set(['nested-arrow', 'shadow-read', 'nested-compound', 'nested-definite']);

async function check(directory, config, source, markers) {
  for (const marker of markers) {
    const label = '/*' + marker + '*/ ';
    const offset = source.indexOf(label);
    assert.ok(offset >= 0, marker);
    const answer = await typeOracle(path.join(directory, config), {
      at: { file: 'boundaries.ts', ...position(lineStarts(source), offset + label.length) }, properties: false,
    });
    assert.equal(answer.compiler, '7.0.2');
    assert.equal(answer.history, 'baseline');
    assert.equal(answer.kind, 'expression', marker);
    assert.equal(answer.summary, changed.has(marker) ? 'string' : scalar.has(marker) ? 'number' : 'string | number', marker);
    assert.equal(answer.errorType, false, marker);
    assert.deepEqual(answer.prelude, []);
  }
}

test('native whole statement endings, nested roots and canonical shadowing', async () => {
  const markers = [...source.matchAll(/\/\*([^*]+)\*\/ /g)].map(match => match[1]);
  assert.equal(markers.length, 27);
  for (const config of ['tsconfig.json', 'loose.json']) await check(directory, config, source, markers);
});

test('native assignment capture keeps raw statement order through each line-break class and BOM', async () => {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'native-capture-boundaries-'));
  try {
    for (const lineBreak of ['\n', '\r', '\r\n', '\u2028', '\u2029']) {
      for (const bom of ['', '\ufeff']) {
        const text = source.replaceAll('\n', lineBreak);
        fs.writeFileSync(path.join(temporary, 'boundaries.ts'), bom + text);
        for (const config of ['tsconfig.json', 'loose.json']) {
          fs.copyFileSync(path.join(directory, config), path.join(temporary, config));
        }
        for (const config of ['tsconfig.json', 'loose.json']) {
          // Native SourceFile.text excludes the leading BOM. Query positions
          // use that accepted text, independently of the captured disk bytes.
          await check(temporary, config, text, ['variable-inside', 'variable-after',
            'if-inside', 'class-inside', 'class-after', 'nested-declaration', 'shadow-read']);
        }
      }
    }
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
});

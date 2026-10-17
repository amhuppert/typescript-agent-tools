import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { compilerHookOracle } from './oracle.mjs';
import { lineStarts, position } from '../../../tools/semantic-oracle.mjs';
const root = path.resolve('fixtures/types/native-signature-mapper');
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const rows = [];
for (const config of ['tsconfig.json', 'loose.json']) {
  for (const name of ['simple', 'genericString', 'genericNumber', 'nested', 'shadow', 'lazy']) {
    const prefix = `declare let ${name}:`;
    const start = source.indexOf(prefix);
    assert.notEqual(start, -1, name);
    const offset = start + 'declare let '.length;
    const request = { at: { file: 'cases.ts', ...position(lineStarts(source), offset) }, properties: false };
    const answer = await compilerHookOracle(path.join(root, config), request);
    rows.push({ config, name, request, answer });
    fs.writeFileSync(new URL('answers.json', import.meta.url), JSON.stringify(rows, null, 2) + '\n');
    console.log(JSON.stringify({ config, name, defaults: answer.signatureDefaults,
      before: answer.beforeDefaults.diagnosticCodes, after: answer.afterDefaults.diagnosticCodes }));
  }
}

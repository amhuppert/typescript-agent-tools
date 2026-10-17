import fs from 'node:fs';
import path from 'node:path';
import { compilerHookOracle } from './oracle.mjs';
import { lineStarts, position } from '../../../tools/semantic-oracle.mjs';
const rows = [];
for (const family of ['basic', 'merged-later-default', 'unused-default']) {
  const directory = path.resolve('fixtures/types/json-generic-targets', family);
  const text = fs.readFileSync(path.join(directory, 'globals.ts'), 'utf8');
  const offset = text.indexOf('<T') + 1;
  if (offset === 0) throw new Error('Missing parameter');
  const request = { at: { file: 'globals.ts', ...position(lineStarts(text), offset) }, properties: false };
  const answer = await compilerHookOracle(path.join(directory, 'strict-1-exact-0-unchecked-0.json'), request);
  rows.push({ family, request, source: text, answer });
  fs.writeFileSync(new URL('answers.json', import.meta.url), JSON.stringify(rows, null, 2) + '\n');
  console.log(JSON.stringify({ family, summary: answer.summary, parameterDefault: answer.parameterDefault,
    before: answer.beforeSummary.diagnosticCodes, after: answer.afterDefault.diagnosticCodes }));
}

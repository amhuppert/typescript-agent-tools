import fs from 'node:fs';
import path from 'node:path';
import { compilerHookOracle, typeOracle } from '../../../tools/type-oracle.mjs';
import { lineStarts, position } from '../../../tools/semantic-oracle.mjs';
const source = `export {};
let uninitialized: number;
/*#uninitialized*/uninitialized;
type UninitializedType = typeof /*#typeQuery*/uninitialized;
/*#nonNull*/uninitialized!;
let count: number = 1;
if (typeof Missing === 'string') { /*#unrelatedMissing*/count; }
interface OnlyType {}
if (typeof OnlyType === 'string') { /*#unrelatedType*/count; }
declare const neverValue: never;
if (typeof Missing === 'string') { /*#neverMissing*/neverValue; }
`;
const here = path.resolve('.cc/temp/types-reader-review');
fs.writeFileSync(path.join(here, 'cases.ts'), source);
const rows = [];
for (const strict of [false, true]) {
  const config = path.join(here, `${strict}.json`);
  fs.writeFileSync(config, JSON.stringify({ compilerOptions: { strict, noLib: true, noEmit: true, target: 'esnext', module: 'esnext' }, files: ['cases.ts'] }));
  for (const marker of ['uninitialized', 'typeQuery', 'nonNull', 'unrelatedMissing', 'unrelatedType', 'neverMissing']) {
    const token = `/*#${marker}*/`;
    const request = { at: { file: 'cases.ts', ...position(lineStarts(source), source.indexOf(token) + token.length) }, properties: false };
    const api = await typeOracle(config, request);
    const hook = await compilerHookOracle(config, request);
    rows.push({ strict, marker, request, api, hook });
    fs.writeFileSync(path.join(here, 'answers.json'), JSON.stringify({ source, rows }, null, 2) + '\n');
    console.log(JSON.stringify({ strict, marker, kind: api.kind, summary: api.summary, errorType: api.errorType,
      hookSummary: hook.summary, diagnostics: hook.beforeSummary.diagnosticCodes }));
  }
}

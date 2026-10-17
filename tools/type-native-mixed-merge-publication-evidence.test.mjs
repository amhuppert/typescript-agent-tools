import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

// Exploratory native evidence only. Scalar expectations must come from the
// captured results and a source ledger before any public regression is staged.
const root = path.resolve('.cc/temp/native-mixed-merge-publication-evidence');
fs.mkdirSync(root, {recursive: true});
const globals = fs.readFileSync('fixtures/types/native-semantic-resources/cases.ts', 'utf8').split('// All demands')[0];
const first = `declare namespace M {
  export import A = M.B;
  namespace A { const priorA: 3; }
  export import B = M.A;
  namespace B { const priorB: 4; }
}
`;
const second = `declare namespace M {
  namespace A { const markerA: 1; }
  namespace B { const markerB: 2; }
}
`;
const source = `declare const /*#bothDirect*/bothDirect: [typeof M.A, typeof M.B] extends [{ markerA: 1; markerB: 2 }, { markerA: 1; markerB: 2 }] ? 41 : 42;
declare const /*#leftAll*/leftAll: typeof M.A extends { markerA: 1; markerB: 2 } ? 51 : 52;
declare const /*#rightAll*/rightAll: typeof M.B extends { markerA: 1; markerB: 2 } ? 61 : 62;
`;
for (const [file, contents] of Object.entries({'globals.d.ts': globals, 'first.d.ts': first,
  'second.d.ts': second, 'consumer.ts': source})) fs.writeFileSync(path.join(root, file), contents);
for (const profile of ['strict', 'nocheck']) fs.writeFileSync(path.join(root, `${profile}.json`),
  JSON.stringify({compilerOptions: {strict: true, noLib: true, noCheck: profile === 'nocheck'},
    files: ['globals.d.ts', 'first.d.ts', 'second.d.ts', 'consumer.ts']}, null, 2) + '\n');

const rows = [];
for (const profile of ['strict', 'nocheck']) {
  test(`native merge publication mixed-alias hypothesis (${profile})`, async () => {
    for (let repeat = 0; repeat < 3; repeat++) {
      for (const name of ['bothDirect', 'leftAll', 'rightAll']) {
        const marker = `/*#${name}*/`;
        const at = {file: 'consumer.ts', ...position(lineStarts(source), source.indexOf(marker) + marker.length)};
        const config = path.join(root, `${profile}.json`);
        const native = await compilerHookOracle(config, {at, history: 'cold', inspectDiagnosticCache: true,
          inspectExpressionAcquisitions: true});
        const api = await typeOracle(config, {at, history: 'cold', properties: false});
        rows.push({profile, repeat, name, at, summary: native.summary, errorType: native.errorType,
          apiSummary: api.summary, apiErrorType: api.errorType,
          codes: native.beforeSummary.diagnosticCodes, failures: native.beforeSummary.demandFailures,
          acquisitions: native.beforeSummary.displayAcquisitions, inputHashes: native.inputHashes});
        fs.writeFileSync('.cc/temp/native-mixed-merge-publication-evidence.json', JSON.stringify(rows, null, 2) + '\n');
        assert.equal(native.compiler, '7.0.2');
        assert.equal(native.checkerCount, 1);
        // Compare each result with a fresh public API checker; do not infer
        // a chosen sibling visitation order from declaration spelling.
        assert.equal(native.errorType, false);
        assert.equal(api.errorType, false);
        assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], `${profile}/${repeat}/${name}`);
      }
    }
  });
}

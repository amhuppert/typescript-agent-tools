import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-generated-literal-output/', import.meta.url));
async function capture(config, file, source) {
  const rows = [];
  for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
    const at = {file, ...position(lineStarts(source), marker.index + marker[0].length)};
    const native = await compilerHookOracle(config, {at, inspectDiagnosticCache: true});
    const api = await typeOracle(config, {at, properties: false});
    assert.equal(native.compiler, '7.0.2');
    assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
    rows.push({name: marker[1], summary: native.summary, errorType: native.errorType,
      codes: native.beforeSummary.diagnosticCodes,
      demandFailures: native.beforeSummary.demandFailures,
      acquisitions: native.beforeSummary.displayAcquisitions ?? []});
  }
  return rows;
}

function baseline(name, rows) {
  fs.mkdirSync('.cc/temp', {recursive: true});
  fs.writeFileSync(`.cc/temp/native-generated-literal-output-${name}.json`, JSON.stringify(rows, null, 2) + '\n');
  assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${name}.json`), 'utf8')));
}

for (const group of [{file: 'cases.ts', prefix: '', count: 7}, {file: 'hidden.ts', prefix: 'hidden-', count: 1}]) {
  const source = fs.readFileSync(path.join(root, group.file), 'utf8');
  for (const mode of ['strict', 'loose']) {
    const profile = `${group.prefix}${mode}`;
    test(`native generated literal output (${profile})`, async () => {
      const rows = await capture(path.join(root, `${profile}.json`), group.file, source);
      assert.equal(rows.length, group.count);
      baseline(profile, rows);
    });
  }
}

// Closed product wire contract, independent of the Rust admission algorithm.
// The declaration starts on its own line so padding never moves its origin.
function budgetWire(padding) {
  return {summary: '…', summaryTruncated: true, expansion: {root: 't0', nodes: [{
    id: 't0', summary: '…', summaryTruncated: true, kind: 'literal', base: 'string',
    value: {parts: [{kind: 'text', value: 'x'.repeat(padding)}, {kind: 'symbol', ref: 's0',
      origins: [{file: 'budget.ts', start: {line: 5, column: 5}, end: {line: 5, column: 10}}]}]},
  }], truncated: true}};
}

const template = fs.readFileSync(path.join(root, 'budget.ts'), 'utf8');
for (const mode of ['strict', 'loose']) {
  test(`native generated literal output budget (${mode})`, async () => {
    const rows = [];
    fs.mkdirSync('.cc/temp', {recursive: true});
    for (const bytes of [262143, 262144, 262145]) {
      const padding = bytes - Buffer.byteLength(JSON.stringify(budgetWire(0)));
      assert.equal(Buffer.byteLength(JSON.stringify(budgetWire(padding))), bytes);
      const source = template.replace('__PADDING__', 'x'.repeat(padding));
      const scratch = fs.mkdtempSync(path.resolve('.cc/temp/literal-output-budget-'));
      try {
        fs.writeFileSync(path.join(scratch, 'budget.ts'), source);
        fs.copyFileSync(path.join(root, '../native-generated-literal-context/globals.d.ts'), path.join(scratch, 'globals.d.ts'));
        const config = path.join(scratch, 'tsconfig.json');
        fs.writeFileSync(config, JSON.stringify({compilerOptions: {strict: mode === 'strict',
          target: 'ESNext', module: 'ESNext', noLib: true, types: []}, files: ['globals.d.ts', 'budget.ts']}));
        const [row] = await capture(config, 'budget.ts', source);
        assert.equal(row.errorType, false);
        assert.deepEqual(row.codes, []);
        rows.push({bytes, padding, ...row});
      } finally {
        fs.rmSync(scratch, {recursive: true, force: true});
      }
    }
    baseline(`budget-${mode}`, rows);
  });
}

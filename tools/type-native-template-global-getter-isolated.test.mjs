import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from '../.cc/temp/stage69-template-global-getter-tooling/tools/type-oracle.mjs';
import {lineStarts, position} from '../.cc/temp/stage69-template-global-getter-tooling/tools/semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/constant-positions/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'positions.ts'), 'utf8');
const offset = marker => source.indexOf(`/*#${marker}*/`) + `/*#${marker}*/`.length;
const rows = [];
const cases = [
  ['argument', 'expression', '"ready"', ['CallExpression']],
  ['nestedCall', 'expression', '"nested"', ['CallExpression', 'CallExpression']],
  ['newArg', 'expression', '"new"', ['NewExpression']],
  ['tagToken', 'expression', '"tagged"', ['TaggedTemplateExpression']],
  ['callbackName', 'value', '1', ['CallExpression']],
  ['callProperty', 'value', 'number', ['CallExpression']],
];

test('isolated native getter preserves actual healthy and missing tagged-call values', async () => {
  // Guard the shared tooling present for this replay, including its inventory.
  // Historical tooling has since been added, split and removed legitimately.
  const snapshot = () => ['tools', 'fixtures/types/compiler-hook'].flatMap(directory =>
    fs.readdirSync(directory).filter(file => /\.(mjs|go)$/.test(file)).sort()
      .map(file => ({file: `${directory}/${file}`,
        sha256: createHash('sha256').update(fs.readFileSync(`${directory}/${file}`)).digest('hex')})));
  const before = snapshot();
  for (const [config, selected] of [['tsconfig.json', cases], ['missing-globals.json', [cases[3]]]]) {
    for (const [marker, kind, summary, prelude] of selected) {
      const at = {file: 'positions.ts', ...position(lineStarts(source), offset(marker))};
      const native = await compilerHookOracle(path.join(root, config), {at, history: 'baseline',
        inspectDiagnosticCache: true, inspectExpressionAcquisitions: true});
      const api = await typeOracle(path.join(root, config), {at, history: 'baseline', properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.equal(native.checkerCount, 1);
      assert.deepEqual([native.kind, native.summary, native.errorType], [kind, summary, false], `${config}/${marker}`);
      assert.deepEqual([native.kind, native.summary, native.errorType], [api.kind, api.summary, api.errorType], `${config}/${marker}`);
      assert.deepEqual(api.prelude.map(row => row.kind), prelude, `${config}/${marker}`);
      const observation = native.beforeSummary.observations.templateStringsGlobalTypeResult;
      const missing = config === 'missing-globals.json';
      if (marker === 'tagToken') {
        assert.ok(observation, `${config}/${marker}: lazy getter must execute in its actual prelude`);
        assert.deepEqual([observation.name, observation.arity, observation.reportErrors,
          observation.nil, observation.emptyObject, observation.emptyGeneric, observation.errorType],
          ['TemplateStringsArray', 0, true, false, missing, false, false], `${config}/${marker}`);
      } else {
        assert.equal(observation, undefined, `${config}/${marker}: unrelated prelude must not query TemplateStringsArray`);
      }
      const errors = native.beforeSummary.displayAcquisitions.filter(event => event.event === 'exit' && event.errorType);
      if (missing) {
        // Go checker offsets are bytes, while API positions are UTF-16.
        const annotation = Buffer.byteLength(source.slice(0, source.indexOf('TemplateStringsArray')), 'utf8');
        assert.ok(errors.some(event => event.operation === 'getTypeFromTypeNode'
          && event.pos <= annotation && event.end === annotation + 'TemplateStringsArray'.length));
        assert.ok(native.beforeSummary.diagnosticCodes.includes(2304));
        assert.ok(native.beforeSummary.diagnosticCodes.includes(2318));
      } else {
        assert.deepEqual(errors, [], `${config}/${marker}`);
      }
      rows.push({config, marker, kind, summary, errorType: native.errorType, prelude: api.prelude,
        globalGetter: observation ?? null, diagnostics: native.beforeSummary.diagnosticCodes,
        errors, sourceCommit: native.sourceCommit, inputHashes: native.inputHashes});
      fs.writeFileSync('.cc/temp/stage69-template-global-getter-evidence.json', JSON.stringify(rows, null, 2) + '\n');
    }
  }
  assert.equal(rows.length, 7);
  assert.deepEqual(snapshot(), before, 'isolated replay preserves shared tooling');
});

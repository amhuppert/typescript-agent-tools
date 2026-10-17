import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position, semanticOracle, resultGroups} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-module-scope/', import.meta.url));
for (const profile of ['strict', 'loose']) {
  test(`native module-scope merge and name boundaries (${profile})`, async () => {
    const rows = [];
    for (const [variant, file] of [
      ['merged-forward', 'merged-b.d.ts'], ['merged-reverse', 'merged-b.d.ts'],
      ['name-failure', 'name-failure.ts'], ['name-control', 'name-control.ts'],
      ['name-unobserved', 'name-unobserved.ts'],
    ]) {
      const config = `${variant}-${profile}.json`;
      const source = fs.readFileSync(path.join(root, file), 'utf8');
      for (const marker of source.matchAll(/\/\*#([^*]+)\*\//g)) {
        const at = {file, ...position(lineStarts(source), marker.index + marker[0].length)};
        const native = await compilerHookOracle(path.join(root, config), {at, inspectDiagnosticCache: true});
        const api = await typeOracle(path.join(root, config), {at, properties: false});
        assert.equal(native.compiler, '7.0.2');
        assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], marker[1]);
        assert.equal(native.errorType, false, marker[1]);
        assert.deepEqual(native.beforeSummary.diagnosticCodes, [], marker[1]);
        assert.deepEqual(native.beforeSummary.demandFailures, [], marker[1]);
        rows.push({config, file, name: marker[1], summary: native.summary, errorType: native.errorType,
          codes: native.beforeSummary.diagnosticCodes, demandFailures: native.beforeSummary.demandFailures});
      }
      if (variant.startsWith('merged-')) {
        const navigation = await semanticOracle(path.join(root, config));
        assert.equal(navigation.compiler, '7.0.2');
        const groups = resultGroups(navigation);
        const sites = new Map(groups.flatMap(group => group.occurrences)
          .filter(occurrence => occurrence.location.file === 'merged-b.d.ts'
            && ['MixedEntry', 'PureEntry'].includes(occurrence.name))
          .map(occurrence => [JSON.stringify(occurrence.location), occurrence.location]));
        const references = [...sites].map(([key, location]) => ({location,
          groups: groups.filter(group => group.occurrences.some(occurrence => JSON.stringify(occurrence.location) === key))}));
        fs.mkdirSync('.cc/temp', {recursive: true});
        fs.writeFileSync(`.cc/temp/native-module-scope-${variant}-${profile}-references.json`, JSON.stringify(references, null, 2) + '\n');
      }
    }
    assert.equal(rows.length, 9);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-module-scope-boundaries-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-boundaries-${profile}.json`), 'utf8')));
    for (const order of ['forward', 'reverse']) {
      const name = `native-module-scope-merged-${order}-${profile}-references.json`;
      assert.deepEqual(JSON.parse(fs.readFileSync(`.cc/temp/${name}`, 'utf8')),
        JSON.parse(fs.readFileSync(path.join(root, `expected-${order}-${profile}-references.json`), 'utf8')));
    }
  });
}

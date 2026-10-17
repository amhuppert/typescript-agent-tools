import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-generated-literal-ordering/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const cases = JSON.parse(fs.readFileSync(path.join(root, 'cases.json'), 'utf8'));

for (const profile of ['strict', 'loose']) {
  test(`native generated literal ordering (${profile})`, async () => {
    const config = path.join(root, `${profile}.json`);
    const rows = [];
    for (const entry of cases) {
      const marker = `/*#${entry.name}*/`;
      const offset = source.indexOf(marker);
      assert.notEqual(offset, -1, entry.name);
      const at = {file: 'cases.ts', ...position(lineStarts(source), offset + marker.length)};
      const native = await compilerHookOracle(config, {at, inspectDiagnosticCache: true});
      const api = await typeOracle(config, {at, properties: false});
      assert.equal(native.compiler, '7.0.2');
      assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], entry.name);
      assert.equal(native.errorType, false, entry.name);
      assert.deepEqual(native.beforeSummary.diagnosticCodes, [], entry.name);
      assert.deepEqual(native.beforeSummary.demandFailures, [], entry.name);
      assert.equal(native.summary[0], '[', entry.name);
      assert.equal(native.summary.at(-1), ']', entry.name);
      let unionSummary = native.summary.slice(1, -1);
      const identities = [];
      for (let index = 0; index < entry.identities; index++) {
        const element = /^("(?:\\.|[^"\\])*"), (.*)$/s.exec(unionSummary);
        assert.ok(element, `${entry.name}: ${native.summary}`);
        const identity = JSON.parse(element[1]);
        assert.match(identity, /^[1-9][0-9]*$/);
        identities.push(identity);
        unionSummary = element[2];
      }
      assert.equal(new Set(identities).size, identities.length, entry.name);
      // Native EscapeString uses JavaScript's \0 spelling, which JSON omits.
      // This fixture's only non-JSON quoted spelling is the isolated NUL value.
      const ordered = unionSummary.split(' | ').map(value => value === '"\\0"' ? '\0' : JSON.parse(value));
      if (entry.ordered !== null) {
        assert.deepEqual(ordered, entry.ordered.map(value => value.replaceAll('$ID', identities[0])), entry.name);
      } else {
        // These native values contain concrete ASCII IDs. Their observed byte
        // order does not authorize the product to invent those spellings.
        assert.equal(ordered.length, 2, entry.name);
        assert.equal(Buffer.compare(Buffer.from(ordered[0]), Buffer.from(ordered[1])), -1, entry.name);
      }
      if (entry.name === 'rawFE') assert.equal(unionSummary, '"�" | "\\uFFFD"');
      rows.push({name: entry.name, summary: native.summary, identities, unionSummary, ordered,
        kind: api.kind, location: api.location, origins: api.origins,
        errorType: native.errorType, codes: native.beforeSummary.diagnosticCodes,
        demandFailures: native.beforeSummary.demandFailures});
    }
    assert.equal(rows.length, 28);
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync(`.cc/temp/native-generated-literal-ordering-${profile}.json`, JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(path.join(root, `expected-${profile}.json`), 'utf8')));
  });
}

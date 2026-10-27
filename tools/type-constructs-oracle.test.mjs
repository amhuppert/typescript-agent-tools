import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {scratchPath} from './harness-scratch.mjs';
import {compilerHookOracle} from './type-oracle.mjs';

// Q5's expected answers: the pinned 7.0.2 checker's contextual types before
// and after object-literal discrimination, declared-type identity, and
// presence read from each literal's full checked type (docs/constructs-design.md).
// One compact public range per literal keeps every examined literal visible.
const span = ({file, start, end}) => `${file}:${start.line}:${start.column}-${end.line}:${end.column}`;

for (const [fixture, configs] of [['constructs', ['tsconfig.json', 'loose.json']], ['constructs-gaps', ['tsconfig.json']], ['constructs-object', ['tsconfig.json']], ['constructs-object-index', ['tsconfig.json']]]) {
  const root = fileURLToPath(new URL(`../fixtures/types/${fixture}/`, import.meta.url));
  const targets = JSON.parse(fs.readFileSync(path.join(root, 'targets.json'), 'utf8'));
  for (const config of configs) {
    test(`construction sites and omitted properties ${fixture} ${config}`, async () => {
      const rows = [];
      for (const target of targets) {
        const answer = await compilerHookOracle(path.join(root, config), {operation: 'constructs', at: target.at});
        assert.equal(answer.compiler, '7.0.2');
        const {literals, ...facts} = answer.constructs;
        rows.push({name: target.name, ...facts, literals: (literals ?? []).map(({location, ...row}) => ({at: span(location), ...row}))});
      }
      // The identity rule's independent floor: a literal whose instantiated
      // context is the declared type or carries its alias is a construction
      // unless discrimination removed it or it is excluded.
      for (const row of rows) for (const literal of row.literals) {
        if (literal.contextIsTarget) assert.ok(literal.constructed?.length || literal.discriminatedAway, `${row.name} ${literal.at}`);
      }
      fs.writeFileSync(scratchPath(`types-${fixture}-${config}`), JSON.stringify(rows, null, 2) + '\n');
      const expected = path.join(root, `expected-${config}`);
      if (fs.existsSync(expected)) assert.deepEqual(rows, JSON.parse(fs.readFileSync(expected, 'utf8')));
      else assert.equal(rows.length, targets.length);
    });
  }
}

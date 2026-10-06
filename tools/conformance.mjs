// Use an installed TypeScript 6 compiler API as the TypeScript 7 syntax oracle.
// TYPE_SCRIPT_API can point to @typescript/typescript6/lib/typescript.js.
import fs from 'node:fs';
import path from 'node:path';
import { COMPARISON_OPTIONS, normalizer, describeComparison } from './ast-comparison.mjs';
import { loadCompilerApi, compareSource } from './reference-oracle.mjs';
import { runnerArguments, requireComparisons } from './corpus-selection.mjs';
const { flags } = runnerArguments(process.argv.slice(2), { options: COMPARISON_OPTIONS });
const normalize = normalizer(flags);
const ts = loadCompilerApi();
const binary = process.env.TSPARSE ?? path.resolve('target/debug/tsparse');
const cases = JSON.parse(fs.readFileSync('fixtures/conformance.json', 'utf8'));
requireComparisons(cases.length);
let failures = 0;
for (const { name, source, jsx } of cases) {
  const facts = compareSource(ts, binary, { name: jsx ? 'source.tsx' : 'source.ts', source, normalize });
  // Authored fixtures are positive cases: TypeScript 7 must still have their
  // syntax and accept it, whatever the parser does with them.
  if (!facts.eligible || !facts.valid) {
    console.log(`INVALID FIXTURE ${name}: ${facts.removed || facts.diagnostics.map(d => d.message).join('; ')}`);
    failures++; continue;
  }
  if (!facts.difference) continue;
  failures++;
  if (facts.outcome !== 'accepted') { console.log(`PARSE FAIL ${name}: ${JSON.stringify(facts.difference)}`); continue; }
  const folder = '.cache/conformance'; fs.mkdirSync(folder, { recursive: true });
  fs.writeFileSync(`${folder}/${name}.actual.json`, JSON.stringify(facts.actual, null, 2));
  fs.writeFileSync(`${folder}/${name}.expected.json`, JSON.stringify(facts.expected, null, 2));
  console.log(`AST FAIL ${name}: ${JSON.stringify(facts.difference)}; compare ${folder}/${name}.{actual,expected}.json`);
}
console.log(`${cases.length - failures}/${cases.length} structural comparisons passed (TypeScript ${ts.version}; compared: ${describeComparison(normalize.compared)})`);
process.exitCode = failures ? 1 : 0;

// Use an installed TypeScript 6 compiler API as the TypeScript 7 syntax oracle.
// TYPE_SCRIPT_API can point to @typescript/typescript6/lib/typescript.js.
import { runParser, parserFailure } from './parser-process.mjs';
import fs from 'node:fs';
import path from 'node:path';
import { normalizer, firstDifference } from './ast-comparison.mjs';
import { loadCompilerApi, classifyReference } from './reference-oracle.mjs';
import { requireComparisons } from './corpus-selection.mjs';
const ts = loadCompilerApi();
const binary = process.env.TSPARSE ?? path.resolve('target/debug/tsparse');
const cases = JSON.parse(fs.readFileSync('fixtures/conformance.json', 'utf8'));
requireComparisons(cases.length);
const normalize = normalizer(process.argv);
let failures = 0;
for (const { name, source, jsx } of cases) {
  const reference = ts.createSourceFile(jsx ? 'source.tsx' : 'source.ts', source, ts.ScriptTarget.Latest, false, jsx ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const classification = classifyReference(ts, reference);
  if (!classification.eligible || !classification.valid) {
    console.log(`INVALID FIXTURE ${name}: ${classification.removed || classification.diagnostics.map(d => d.message).join('; ')}`);
    failures++; continue;
  }
  const result = runParser(binary, jsx ? ['--tsx'] : [], { input: source });
  if (result.outcome !== 'accepted') { console.log(`PARSE FAIL ${name}: ${JSON.stringify(parserFailure(result))}`); failures++; continue; }
  const actual = normalize(JSON.parse(result.stdout), true);
  const expected = normalize(reference, true);
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    const folder = '.cache/conformance'; fs.mkdirSync(folder, { recursive: true });
    fs.writeFileSync(`${folder}/${name}.actual.json`, JSON.stringify(actual, null, 2));
    fs.writeFileSync(`${folder}/${name}.expected.json`, JSON.stringify(expected, null, 2));
    console.log(`AST FAIL ${name}: ${JSON.stringify(firstDifference(actual, expected))}; compare ${folder}/${name}.{actual,expected}.json`); failures++;
  }
}
console.log(`${cases.length - failures}/${cases.length} structural comparisons passed (TypeScript ${ts.version})`);
process.exitCode = failures ? 1 : 0;

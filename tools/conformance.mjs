// Use an installed TypeScript 6 compiler API as the TypeScript 7 syntax oracle.
// TYPE_SCRIPT_API can point to @typescript/typescript6/lib/typescript.js.
import { runParser, parserFailure } from './parser-process.mjs';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { normalizer, firstDifference } from './ast-comparison.mjs';
const require = createRequire(import.meta.url);
const ts = require(process.env.TYPE_SCRIPT_API ?? 'typescript');
const binary = process.env.TSPARSE ?? path.resolve('target/debug/tsparse');
const cases = JSON.parse(fs.readFileSync('fixtures/conformance.json', 'utf8'));
const normalize = normalizer(process.argv);
let failures = 0;
for (const { name, source, jsx } of cases) {
  const reference = ts.createSourceFile(jsx ? 'source.tsx' : 'source.ts', source, ts.ScriptTarget.Latest, false, jsx ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  if (reference.parseDiagnostics.some(d => d.code !== 2458)) {
    console.log(`INVALID FIXTURE ${name}: ${reference.parseDiagnostics.map(d=>ts.flattenDiagnosticMessageText(d.messageText, '\n')).join('; ')}`);
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

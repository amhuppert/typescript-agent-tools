// Test an upstream compiler checkout without adding its fixtures to this repo.
import { runParser, parserFailure } from './parser-process.mjs';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { normalizer, firstDifference } from './ast-comparison.mjs';
import { walk, units, corpusFolders } from './upstream-fixtures.mjs';
import { loadCompilerApi, classifyReference } from './reference-oracle.mjs';
import { corpusLimit, requireComparisons } from './corpus-selection.mjs';
const ts = loadCompilerApi();
const binary = process.env.TSPARSE ?? path.resolve('target/release/tsparse');
const args = process.argv.slice(2);
const root = args.find(arg => !arg.startsWith('--'));
if (!root) throw new Error('Usage: node tools/upstream-corpus.mjs CHECKOUT [--limit=N] [--match=REGEX] [--check-only] [--positions --flags --scanner-hints]');
const limit = corpusLimit(args);
const match = new RegExp(args.find(arg => arg.startsWith('--match='))?.slice(8) ?? '');
const checkOnly = args.includes('--check-only');
const failuresReport = args.find(arg => arg.startsWith('--failures='))?.slice(11);
const selectedUnits = failuresReport ? new Set(JSON.parse(fs.readFileSync(failuresReport, 'utf8')).failures
  .map(test => `${test.fixture}\0${test.name}`)) : undefined;
const reportFile = args.find(arg => arg.startsWith('--report='))?.slice(9) ?? '.cache/upstream-corpus.json';
const normalize = normalizer(args);
const directory = path.resolve('.cache/upstream-units'); fs.mkdirSync(directory, { recursive: true });
const failures = [];
const commit = spawnSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout.trim();
function saveReport(complete) {
  const report = { compilerApi: ts.version, commit, complete, scope: { limit: Number.isFinite(limit) ? limit : null, match: match.source, failuresReport }, count, valid, invalid, removed, passed, bytes, failures };
  fs.writeFileSync(reportFile, JSON.stringify(report, null, 2));
  return report;
}
let count = 0, valid = 0, invalid = 0, passed = 0, bytes = 0, removed = 0;
outer: for (const folder of corpusFolders(root)) {
  for (const fixture of walk(folder)) {
    if (!match.test(fixture)) continue;
    for (const { name, source } of units(fixture, fs.readFileSync(fixture, 'utf8'))) {
      if (selectedUnits && !selectedUnits.has(`${path.relative(root, fixture)}\0${name}`)) continue;
      if (count >= limit) break outer;
      const jsx = name.endsWith('.tsx');
      const reference = ts.createSourceFile(name, source, ts.ScriptTarget.Latest, false, jsx ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
      const classification = classifyReference(ts, reference);
      if (!classification.eligible) { removed++; continue; }
      count++; bytes += Buffer.byteLength(source);
      const { diagnostics, valid: referenceValid } = classification;
      if (referenceValid) valid++; else invalid++;
      // Keep the extension, including .d.mts/.d.cts, for the Rust ambient context.
      const file = path.join(directory, `${count}-${name.replace(/[^\w.\-]/g, '_')}`);
      fs.writeFileSync(file, source);
      const result = runParser(binary, checkOnly || !referenceValid ? ['--check', file] : [file]);
      let difference;
      if (result.outcome === 'failed') difference = parserFailure(result);
      else if (!referenceValid && result.outcome === 'accepted') difference = { error: 'Accepted compiler-invalid syntax', diagnostics };
      else if (referenceValid && result.outcome !== 'accepted') difference = parserFailure(result);
      else if (referenceValid && !checkOnly) difference = firstDifference(normalize(JSON.parse(result.stdout), true), normalize(reference, true));
      if (difference) {
        failures.push({ fixture: path.relative(root, fixture), name, file, ...difference });
        if (failures.length <= 20) console.log(JSON.stringify(failures.at(-1)));
      } else { passed++; fs.unlinkSync(file); }
      if (count % 1000 === 0) {
        saveReport(false);
        console.log(JSON.stringify({ progress: count, valid, invalid, passed, failures: failures.length }));
      }
    }
  }
}
requireComparisons(count);
const report = saveReport(true);
console.log(JSON.stringify({ ...report, failures: failures.length }));
process.exitCode = failures.length ? 1 : 0;

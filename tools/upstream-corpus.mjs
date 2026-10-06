// Test an upstream compiler checkout without adding its fixtures to this repo.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { COMPARISON_OPTIONS, normalizer, describeComparison } from './ast-comparison.mjs';
import { walk, units, corpusFolders } from './upstream-fixtures.mjs';
import { loadCompilerApi, compareSource } from './reference-oracle.mjs';
import { runnerArguments, corpusLimit, requireComparisons } from './corpus-selection.mjs';
const args = process.argv.slice(2);
const { flags, values, paths: [root] } = runnerArguments(args, { positional: 1,
  options: [...COMPARISON_OPTIONS, '--check-only', '--limit=', '--match=', '--failures=', '--report='] });
if (!root) throw new Error('Usage: node tools/upstream-corpus.mjs CHECKOUT [--limit=N] [--match=REGEX] [--failures=REPORT] [--report=FILE] [--check-only] [--positions --flags --scanner-hints]');
const limit = corpusLimit(args);
const match = new RegExp(values.get('--match=') ?? '');
const checkOnly = flags.has('--check-only');
const failuresReport = values.get('--failures=');
const selectedUnits = failuresReport ? new Set(JSON.parse(fs.readFileSync(failuresReport, 'utf8')).failures
  .map(test => `${test.fixture}\0${test.name}`)) : undefined;
const reportFile = values.get('--report=') ?? '.cache/upstream-corpus.json';
const normalize = normalizer(flags);
const compared = describeComparison(normalize.compared, { trees: !checkOnly });
const ts = loadCompilerApi();
const binary = process.env.TSPARSE ?? path.resolve('target/release/tsparse');
const directory = path.resolve('.cache/upstream-units'); fs.mkdirSync(directory, { recursive: true });
const failures = [];
const commit = spawnSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout.trim();
function saveReport(complete) {
  const report = { compilerApi: ts.version, compared, commit, complete, scope: { limit: Number.isFinite(limit) ? limit : null, match: match.source, failuresReport }, count, valid, invalid, removed, passed, bytes, failures };
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
      // Keep the extension, including .d.mts/.d.cts, for the Rust ambient context.
      const file = path.join(directory, `${count + 1}-${name.replace(/[^\w.\-]/g, '_')}`);
      fs.writeFileSync(file, source);
      const facts = compareSource(ts, binary, { name, source, file, normalize, checkOnly });
      // Syntax removed in TypeScript 7 is excluded; an invalid unit passes when rejected.
      if (!facts.eligible) { removed++; fs.unlinkSync(file); continue; }
      count++; bytes += Buffer.byteLength(source);
      if (facts.valid) valid++; else invalid++;
      if (facts.difference) {
        failures.push({ fixture: path.relative(root, fixture), name, file, ...facts.difference });
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

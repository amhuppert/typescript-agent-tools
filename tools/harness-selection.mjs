#!/usr/bin/env node
// Which harness test files a set of changed paths can affect, for the changed
// harness-test wrapper. Usage:
//   node tools/harness-selection.mjs CHANGED
// CHANGED holds NUL-separated paths relative to the working directory, the
// project root. Prints `every<TAB>PATH` when PATH may affect any test, and
// otherwise each selected tools/*.test.mjs file on its own line (none when no
// test can observe the change).
//
// A test file's closure is the file and every module it reaches through a
// relative import or a relative module path it names. Each changed path
// selects what can read it:
// - tools/NAME.test.mjs: that test, or nothing once it is deleted;
// - another JavaScript module under tools/: the tests whose closure holds it,
//   and, when a validation script names it, the tests that name scripts/;
// - Rust sources and build inputs (src/, tests/, examples/, benches/,
//   vendor/, build.rs, Cargo.toml, Cargo.lock, .cargo/): the tests whose
//   closure names the Rust tool, Cargo, a build directory, or a Rust path;
// - files no module imports (scripts/, docs/, memory-bank/, and top-level
//   documents and settings), and other files under tools/: the tests whose
//   closure names the directory or file;
// - anything else, such as fixtures/ and package files: every test.
import fs from 'node:fs';
import path from 'node:path';

const RUST = /tsparse|cargo|reference_batch|target\/|\bsrc\/|\btests\/|vendor\/|build\.rs|\.rs\b/i;
const MODULE = /\.(?:mjs|cjs|js)$/;
const REFERENCE = /['"`]((?:\.{1,2}\/|tools\/)[^'"`\s]+\.(?:mjs|cjs|js))['"`]/g;

const changed = fs.readFileSync(process.argv[2], 'utf8').split('\0').filter(Boolean);
const tests = fs.readdirSync('tools')
  .filter(name => name.endsWith('.test.mjs') && fs.statSync(path.join('tools', name)).isFile())
  .map(name => `tools/${name}`)
  .sort();

// Each module's closure: the project-relative paths it reaches and its text.
const closures = new Map();
function closure(file) {
  if (closures.has(file)) return closures.get(file);
  const result = { files: new Set([file]), text: '' };
  closures.set(file, result);
  if (!fs.existsSync(file)) return result;
  const text = fs.readFileSync(file, 'utf8');
  result.text = text;
  for (const [, reference] of text.matchAll(REFERENCE)) {
    const target = reference.startsWith('tools/')
      ? reference
      : path.relative('.', path.resolve(path.dirname(file), reference));
    if (target.startsWith('..')) continue;
    const reached = closure(target);
    for (const reachedFile of reached.files) result.files.add(reachedFile);
    result.text += '\n' + reached.text;
  }
  return result;
}

const scriptsName = name => fs.existsSync('scripts') && fs.readdirSync('scripts', { recursive: true })
  .map(entry => path.join('scripts', String(entry)))
  .some(entry => fs.statSync(entry).isFile() && fs.readFileSync(entry, 'utf8').includes(name));

const selected = new Set();
const mentioning = needle => {
  const found = tests.filter(test => closure(test).text.includes(needle));
  for (const test of found) selected.add(test);
  return found.length;
};
for (const file of changed) {
  if (/^tools\/[^/]+\.test\.mjs$/.test(file)) {
    if (fs.existsSync(file)) selected.add(file);
  } else if (file.startsWith('tools/') && MODULE.test(file)) {
    for (const test of tests) if (closure(test).files.has(file)) selected.add(test);
    // The validation scripts run some modules, such as the test reporter.
    if (scriptsName(path.basename(file))) mentioning('scripts/');
  } else if (/^(?:src|tests|examples|benches|vendor|\.cargo)\//.test(file)
    || ['build.rs', 'Cargo.toml', 'Cargo.lock'].includes(file)) {
    for (const test of tests) if (RUST.test(closure(test).text)) selected.add(test);
  } else if (/^(?:scripts|docs|memory-bank)\//.test(file)) {
    mentioning(file.slice(0, file.indexOf('/') + 1));
  } else if (['README.md', 'AGENTS.md', 'CommandCenter.json', 'LICENSE', '.gitignore'].includes(file)) {
    mentioning(file);
  } else if (file.startsWith('tools/')) {
    // A data file a module reads by name; one no test names may be read by a
    // computed path, so it may affect any test.
    if (mentioning(path.basename(file)) === 0) {
      console.log(`every\t${file}`);
      process.exit(0);
    }
  } else {
    console.log(`every\t${file}`);
    process.exit(0);
  }
}
for (const test of [...selected].sort()) console.log(test);

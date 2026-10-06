import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadCompilerApi, nativeCompilerVersion, classifyReference, compareSource, nativeDiagnostics } from './reference-oracle.mjs';
import { corpusLimit, runnerArguments } from './corpus-selection.mjs';
import { COMPARISON_OPTIONS, normalizer } from './ast-comparison.mjs';

function temporary(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-oracle-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  return directory;
}

// This deliberately small API models the documented deferred grammar errors.
// It tests harness contracts, rather than asserting compiler compatibility.
function syntheticApi(directory, { version = '6.0.3', allInvalid = false } = {}) {
  const file = path.join(directory, `api-${version}-${allInvalid}.cjs`);
  fs.writeFileSync(file, `module.exports = {
    version: ${JSON.stringify(version)},
    ScriptTarget: {Latest: 99}, ScriptKind: {TS: 3, TSX: 4},
    NodeFlags: {Namespace: 32, GlobalAugmentation: 2048, NestedNamespace: 8},
    SyntaxKind: {AssertKeyword: 132, JSDocUnknownType: 312, JSDocFunctionType: 317},
    createSourceFile(name, source) {
      const children = [];
      if (source.includes('throw\\n')) children.push({kind: 'throw', expression: {kind: 'identifier', escapedText: '', pos: 5, end: 5}});
      if (source.includes('/*unquoted-module*/')) children.push({kind: 'module', name: {kind: 'identifier'}, flags: 0});
      if (source.includes('/*assert-attributes*/')) children.push({kind: 'import', attributes: {token: 132}});
      if (source.includes('/*legacy-type*/')) children.push({kind: 312});
      const parseDiagnostics = ${allInvalid} || source.includes('/*invalid*/') ? [{code: 1000, messageText: 'Invalid syntax'}]
        : source.includes('/*amd-name*/') ? [{code: 2458, messageText: 'Duplicate AMD name'}] : [];
      return {kind: 308, children, parseDiagnostics};
    },
    isModuleDeclaration: node => node.kind === 'module',
    isStringLiteral: node => node.kind === 'string',
    isImportDeclaration: node => node.kind === 'import',
    isExportDeclaration: node => node.kind === 'export',
    isImportTypeNode: node => node.kind === 'import-type',
    isThrowStatement: node => node.kind === 'throw',
    isIdentifier: node => node.kind === 'identifier',
    forEachChild: (node, callback) => (node.children || []).forEach(callback),
    flattenDiagnosticMessageText: text => text,
  };`);
  return file;
}

function executable(directory, name, body) {
  const file = path.join(directory, name);
  fs.writeFileSync(file, '#!/bin/sh\n' + body + '\n', { mode: 0o755 });
  return file;
}

function runner(directory, name, args = [], environment = {}) {
  return spawnSync(process.execPath, [fileURLToPath(new URL(`./${name}.mjs`, import.meta.url)), ...args], {
    cwd: directory, env: { ...process.env, ...environment }, encoding: 'utf8', timeout: 30_000,
  });
}

function fixture(directory, name, source) {
  const folder = path.join(directory, 'checkout/tests/cases/compiler');
  fs.mkdirSync(folder, { recursive: true });
  fs.writeFileSync(path.join(folder, name), source);
  return path.join(directory, 'checkout');
}

test('synthetic API: shared TS7 classification handles eligibility and deferred diagnostics', t => {
  const directory = temporary(t);
  const ts = loadCompilerApi(syntheticApi(directory));
  const classify = source => classifyReference(ts, ts.createSourceFile('source.ts', source));
  assert.equal(classify('const x = 1;').valid, true);
  assert.equal(classify('/*amd-name*/').valid, true);
  assert.deepEqual(classify('throw\nnew Error();').diagnostics.map(d => d.code), [1142]);
  assert.equal(classify('/*invalid*/').valid, false);
  for (const [source, removed] of [
    ['/*unquoted-module*/', 'unquoted module'],
    ['/*assert-attributes*/', 'assert attributes'],
    ['/*legacy-type*/', 'legacy JSDoc type syntax'],
  ]) {
    assert.equal(classify(source).eligible, false);
    assert.equal(classify(source).removed, removed);
  }
  assert.deepEqual(classify('/*unquoted-module*/').strictDiagnostics.map(d => d.code), [1540]);
  for (const flags of Object.values(ts.NodeFlags)) {
    const reference = ts.createSourceFile('source.ts', '/*unquoted-module*/');
    reference.children[0].flags = flags;
    assert.equal(classifyReference(ts, reference).eligible, true);
  }
});

// A stand-in Rust CLI that records its arguments and stdin, then prints the
// given output and exits with the given status.
function parserStandIn(directory) {
  const binary = path.join(directory, 'parser');
  const log = path.join(directory, 'parser-calls.jsonl');
  return {
    binary,
    behave({ status = 0, stdout = '', stderr = '' } = {}) {
      fs.writeFileSync(log, '');
      fs.writeFileSync(binary, `#!${process.execPath}
const fs = require('node:fs');
fs.appendFileSync(${JSON.stringify(log)}, JSON.stringify({ args: process.argv.slice(2), stdin: fs.readFileSync(0, 'utf8') }) + '\\n');
process.stdout.write(${JSON.stringify(stdout)});
process.stderr.write(${JSON.stringify(stderr)});
process.exitCode = ${status};
`, { mode: 0o755 });
    },
    calls: () => fs.readFileSync(log, 'utf8').split('\n').filter(Boolean).map(line => JSON.parse(line)),
  };
}

test('compareSource reports agreement and each way the parser can disagree with the oracle', t => {
  const directory = temporary(t);
  const ts = loadCompilerApi(syntheticApi(directory));
  const parser = parserStandIn(directory);
  const normalize = normalizer();
  const tree = JSON.stringify({ kind: 308, children: [] });
  const compare = (source, behavior) => {
    parser.behave(behavior);
    return compareSource(ts, parser.binary, { name: 'source.ts', source, normalize });
  };
  const valid = 'const x = 1;', invalid = '/*invalid*/';

  const agreed = compare(valid, { stdout: tree });
  assert.deepEqual([agreed.eligible, agreed.valid, agreed.outcome, agreed.difference], [true, true, 'accepted', undefined]);
  assert.deepEqual(parser.calls(), [{ args: [], stdin: valid }]);

  const differs = compare(valid, { stdout: JSON.stringify({ kind: 308, children: [{ kind: 80 }] }) });
  assert.deepEqual(differs.difference, { path: '$.children.length', actual: 1, expected: 0 });
  assert.deepEqual([differs.actual, differs.expected], [{ kind: 308, children: [{ kind: 80 }] }, { kind: 308, children: [] }]);

  const rejectedValid = compare(valid, { status: 1, stderr: 'syntax error at 1:1' });
  assert.equal(rejectedValid.outcome, 'rejected');
  assert.deepEqual(rejectedValid.difference, { error: 'syntax error at 1:1', status: 1, signal: null });

  const rejectedInvalid = compare(invalid, { status: 1, stderr: 'syntax error' });
  assert.deepEqual([rejectedInvalid.valid, rejectedInvalid.outcome, rejectedInvalid.difference], [false, 'rejected', undefined]);
  assert.deepEqual(parser.calls(), [{ args: ['--check'], stdin: invalid }]);

  const acceptedInvalid = compare(invalid, {});
  assert.deepEqual(acceptedInvalid.difference,
    { error: 'Accepted compiler-invalid syntax', diagnostics: [{ code: 1000, message: 'Invalid syntax' }] });

  for (const source of [valid, invalid]) {
    const failed = compare(source, { status: 2, stdout: tree, stderr: 'cannot read input' });
    assert.equal(failed.outcome, 'failed');
    assert.deepEqual(failed.difference, { error: 'cannot read input', status: 2, signal: null });
  }

  const removed = compare('/*unquoted-module*/', { stdout: tree });
  assert.deepEqual([removed.eligible, removed.removed, removed.outcome, removed.difference],
    [false, 'unquoted module', undefined, undefined]);
  assert.deepEqual(parser.calls(), []);
});

test('compareSource parses a named file or stdin, and checks without a tree under checkOnly', t => {
  const directory = temporary(t);
  const ts = loadCompilerApi(syntheticApi(directory));
  const parser = parserStandIn(directory);
  const normalize = normalizer();
  const tree = JSON.stringify({ kind: 308, children: [] });
  const source = 'const x = 1;';
  const file = path.join(directory, 'unit.d.ts');
  fs.writeFileSync(file, source);

  parser.behave({ stdout: tree });
  assert.equal(compareSource(ts, parser.binary, { name: 'source.tsx', source, normalize }).difference, undefined);
  assert.deepEqual(parser.calls(), [{ args: ['--tsx'], stdin: source }]);

  parser.behave({ stdout: tree });
  assert.equal(compareSource(ts, parser.binary, { name: 'unit.d.ts', source, file, normalize }).difference, undefined);
  assert.deepEqual(parser.calls(), [{ args: [file], stdin: '' }]);

  parser.behave();
  const checked = compareSource(ts, parser.binary, { name: 'unit.d.ts', source, file, checkOnly: true });
  assert.deepEqual([checked.outcome, checked.difference], ['accepted', undefined]);
  assert.deepEqual(parser.calls(), [{ args: ['--check', file], stdin: '' }]);

  parser.behave({ status: 1 });
  assert.equal(compareSource(ts, parser.binary, { name: 'unit.d.ts', source: '/*invalid*/', file, normalize }).difference, undefined);
  assert.deepEqual(parser.calls().map(call => call.args), [['--check', file]]);
});

test('oracle identity requires the pinned API and a successful native TS7 version response', t => {
  const directory = temporary(t);
  assert.throws(() => loadCompilerApi(syntheticApi(directory, { version: '6.0.2' })), /Expected compiler API 6\.0\.3/);
  const native = executable(directory, 'native', 'echo "Version 7.0.2"');
  assert.equal(nativeCompilerVersion(native), 'Version 7.0.2');
  for (const body of ['echo "Version 6.0.3"', 'echo "Version 7.0.2"; exit 2', 'exit 2']) {
    executable(directory, 'native', body);
    assert.throws(() => nativeCompilerVersion(native), /working TypeScript 7 compiler/);
  }
  assert.throws(() => nativeCompilerVersion(path.join(directory, 'missing')), /working TypeScript 7 compiler/);
});

test('native diagnostic exits remain distinct from Rust exits and setup failures', () => {
  const result = { status: 2, signal: null, stdout: 'source.ts(1,2): error TS1000: Syntax error\n', stderr: '' };
  assert.deepEqual(nativeDiagnostics(result), [{ file: path.resolve('source.ts'), line: 1, column: 2, code: 1000, message: 'Syntax error' }]);
  assert.deepEqual(nativeDiagnostics({ ...result, status: 0, stdout: '' }), []);
  for (const failure of [
    { ...result, stdout: 'error TS5023: Unknown option\n' },
    { ...result, stdout: result.stdout + 'error TS5023: Unknown option\n' },
    { ...result, stdout: '', stderr: 'Configuration failed' },
    { ...result, status: null, signal: 'SIGKILL' },
    { ...result, error: new Error('timed out') },
  ]) assert.throws(() => nativeDiagnostics(failure), /Native/);
});

test('explicit corpus limits must select a positive finite integer number of cases', () => {
  assert.equal(corpusLimit([]), Infinity);
  assert.equal(corpusLimit(['--limit=3']), 3);
  assert.equal(corpusLimit([], 0x110000, 0x110000), 0x110000);
  for (const value of ['', '0', '-1', '1.5', 'NaN', 'Infinity', '9007199254740992'])
    assert.throws(() => corpusLimit([`--limit=${value}`]), /positive integer/);
  assert.throws(() => corpusLimit(['--limit=1114113'], 0x110000, 0x110000), /no greater than/);
});

test('runner arguments accept only the options and positional arguments a runner declares', () => {
  const options = ['--positions', '--check-only', '--limit=', '--match='];
  assert.deepEqual(runnerArguments(['root', '--positions', '--limit=3', '--match=', '--positions'], { options, positional: 1 }),
    { flags: new Set(['--positions']), values: new Map([['--limit=', '3'], ['--match=', '']]), paths: ['root'] });
  assert.deepEqual(runnerArguments(['-', 'a', 'b'], { positional: Infinity }).paths, ['-', 'a', 'b']);
  for (const arg of ['--positons', '--position', '--Positions', '-positions', '--positions=1', '--check-only=yes', '--limit', '--'])
    assert.throws(() => runnerArguments([arg], { options }), error => error.message.startsWith(`Unknown option ${arg}; this runner accepts --positions --check-only --limit= --match=`), arg);
  assert.throws(() => runnerArguments(['--positions'], {}), /Unknown option --positions; this runner accepts no options/);
  assert.throws(() => runnerArguments(['--limit=1', '--limit=2'], { options }), /--limit is given more than once/);
  assert.throws(() => runnerArguments(['a', 'b'], { options, positional: 1 }), /Unexpected argument b/);
  assert.throws(() => runnerArguments(['a'], { options }), /Unexpected argument a/);
});

test('synthetic harnesses reject wrong layouts, empty selections, and invalid limits', t => {
  const directory = temporary(t);
  const api = syntheticApi(directory);
  const parser = executable(directory, 'parser', 'exit 1');
  const native = executable(directory, 'native', 'echo "Version 7.0.2"');
  const environment = { TYPE_SCRIPT_API: api, TYPE_SCRIPT_NATIVE: native, TSPARSE: parser };
  const empty = path.join(directory, 'empty'); fs.mkdirSync(empty);
  const checkout = fixture(directory, 'invalid.ts', 'throw\nnew Error();');
  fs.mkdirSync(path.join(directory, 'fixtures'));
  fs.writeFileSync(path.join(directory, 'fixtures/conformance.json'), '[]');
  const probes = [
    ['upstream-corpus', [empty], /No upstream corpus/],
    ['upstream-native-corpus', [empty], /No upstream corpus/],
    ['upstream-corpus', [checkout, '--match=never-matches'], /No cases were compared/],
    ['upstream-corpus', [checkout, '--limit=0'], /positive integer/],
    ['upstream-native-corpus', [checkout, '--limit=NaN'], /positive integer/],
    ['source-corpus', [empty], /No cases were compared/],
    ['declaration-corpus', [empty], /No cases were compared/],
    ['robustness-corpus', ['--limit=-1'], /positive integer/],
    ['unicode-corpus', ['--limit=1.5'], /positive integer/],
    ['conformance', [], /No cases were compared/],
  ];
  for (const [name, args, message] of probes) {
    const result = runner(directory, name, args, environment);
    assert.equal(result.error, undefined, name);
    assert.notEqual(result.status, 0, name);
    assert.match(result.stderr, message, name);
  }
});

test('synthetic source/upstream checks count eligible syntax rejections and skip removed syntax', t => {
  const directory = temporary(t);
  const environment = { TYPE_SCRIPT_API: syntheticApi(directory), TSPARSE: executable(directory, 'parser', 'exit 1') };
  const sourceDirectory = path.join(directory, 'sources'); fs.mkdirSync(sourceDirectory);
  for (const [name, source] of [['throw.ts', 'throw\nnew Error();'], ['removed.ts', '/*unquoted-module*/']]) {
    fs.writeFileSync(path.join(sourceDirectory, name), source);
    fixture(directory, name, source);
  }
  for (const [name, args, reportFile] of [
    ['source-corpus', [sourceDirectory], 'source-corpus.json'],
    ['upstream-corpus', [path.join(directory, 'checkout'), '--check-only'], 'upstream-corpus.json'],
  ]) {
    const result = runner(directory, name, args, environment);
    assert.equal(result.status, 0, result.stdout + result.stderr);
    const report = JSON.parse(fs.readFileSync(path.join(directory, '.cache', reportFile), 'utf8'));
    assert.equal(report.invalid, 1);
    assert.equal(report.removed, 1);
    assert.equal(report.passed, 1);
    assert.deepEqual(report.failures, []);
    executable(directory, 'parser', 'exit 2');
    const failed = runner(directory, name, args, environment);
    assert.equal(failed.status, 1, failed.stdout + failed.stderr);
    const failedReport = JSON.parse(fs.readFileSync(path.join(directory, '.cache', reportFile), 'utf8'));
    assert.equal(failedReport.passed, 0);
    assert.equal(failedReport.failures[0].status, 2);
    executable(directory, 'parser', 'exit 1');
  }
});

test('synthetic runners apply their own policy to the same comparison facts', t => {
  const directory = temporary(t);
  const environment = { TYPE_SCRIPT_API: syntheticApi(directory), TSPARSE: path.join(directory, 'parser') };
  // Rejects sources containing "invalid"; otherwise prints a tree with one
  // extra child for sources containing "extra".
  fs.writeFileSync(environment.TSPARSE, `#!${process.execPath}
const fs = require('node:fs');
const file = process.argv.slice(2).find(arg => !arg.startsWith('--'));
const source = fs.readFileSync(file ?? 0, 'utf8');
if (source.includes('invalid')) { process.stderr.write('syntax error'); process.exitCode = 1; }
else if (!process.argv.includes('--check')) process.stdout.write(JSON.stringify({ kind: 308, children: source.includes('extra') ? [{ kind: 80 }] : [] }));
`, { mode: 0o755 });
  const cases = [['same', 'const x = 1;'], ['extra', 'const extra = 1;'], ['invalid', '/*invalid*/']];
  fs.mkdirSync(path.join(directory, 'fixtures'));
  fs.writeFileSync(path.join(directory, 'fixtures/conformance.json'), JSON.stringify(cases.map(([name, source]) => ({ name, source }))));
  const authored = runner(directory, 'conformance', [], environment);
  assert.equal(authored.status, 1, authored.stderr);
  assert.deepEqual(authored.stdout.trim().split('\n'), [
    'AST FAIL extra: {"path":"$.children.length","actual":1,"expected":0}; compare .cache/conformance/extra.{actual,expected}.json',
    'INVALID FIXTURE invalid: Invalid syntax',
    '1/3 structural comparisons passed (TypeScript 6.0.3; compared: structure only)',
  ]);
  const dump = JSON.parse(fs.readFileSync(path.join(directory, '.cache/conformance/extra.actual.json'), 'utf8'));
  assert.deepEqual(dump, { kind: 308, children: [{ kind: 80 }] });

  // A corpus runner passes the same rejected invalid input.
  const sources = path.join(directory, 'sources'); fs.mkdirSync(sources);
  for (const [name, source] of cases) fs.writeFileSync(path.join(sources, `${name}.ts`), source);
  const corpus = runner(directory, 'source-corpus', [sources], environment);
  assert.equal(corpus.status, 1, corpus.stderr);
  const report = JSON.parse(fs.readFileSync(path.join(directory, '.cache/source-corpus.json'), 'utf8'));
  assert.deepEqual([report.total, report.invalid, report.passed], [3, 1, 2]);
  assert.deepEqual(report.failures.map(failure => [path.basename(failure.file), failure.path]), [['extra.ts', '$.children.length']]);
});

// Misspelled comparison flags once compared less while reporting the same pass.
test('syntax runners reject options they do not read before loading a compiler', t => {
  const directory = temporary(t);
  const checkout = fixture(directory, 'ok.ts', 'const x = 1;');
  const sources = path.join(directory, 'sources'); fs.mkdirSync(sources);
  fs.writeFileSync(path.join(sources, 'ok.ts'), 'const x = 1;');
  // A missing API proves that option errors come first.
  const environment = { TYPE_SCRIPT_API: path.join(directory, 'missing-api.cjs'), TSPARSE: executable(directory, 'parser', 'exit 0') };
  const misspelled = ['--positons', '--flgs', '--scanner-hint', '--position', '-positions', '--positions=1'];
  for (const [name, args, foreign] of [
    ['conformance', [], ['--check-only', '--limit=1', 'path']],
    ['grammar-corpus', [], ['--check-only', '--limit=1', 'path']],
    ['source-corpus', [sources], ['--check-only', '--limit=1', '--match=ok']],
    ['upstream-corpus', [checkout], ['--check', '--limit', '--matches=ok', '--report', 'second-checkout']],
  ]) {
    for (const option of [...misspelled, ...foreign]) {
      const result = runner(directory, name, [...args, '--flags', option], environment);
      assert.equal(result.status, 1, `${name} ${option}: ${result.stdout}${result.stderr}`);
      assert.match(result.stderr, option.startsWith('-') ? new RegExp(`Unknown option ${option}; this runner accepts --positions --flags --scanner-hints`)
        : new RegExp(`Unexpected argument ${option}`), `${name} ${option}`);
    }
  }
  assert.equal(fs.existsSync(path.join(directory, '.cache')), false);
});

test('syntax runners apply each comparison option they accept and report the active set', t => {
  const directory = temporary(t);
  const environment = { TYPE_SCRIPT_API: syntheticApi(directory),
    // The synthetic API's tree has no positions, flags, or scanner hints.
    TSPARSE: executable(directory, 'parser', `printf '%s' '${JSON.stringify({ kind: 308, children: [], pos: 0, end: 1, flags: 16, rawText: 'r' })}'`) };
  fs.mkdirSync(path.join(directory, 'fixtures'));
  fs.writeFileSync(path.join(directory, 'fixtures/conformance.json'), JSON.stringify([{ name: 'case', source: 'const x = 1;' }]));
  const sources = path.join(directory, 'sources'); fs.mkdirSync(sources);
  fs.writeFileSync(path.join(sources, 'case.ts'), 'const x = 1;');
  const checkout = fixture(directory, 'case.ts', 'const x = 1;');
  const report = file => JSON.parse(fs.readFileSync(path.join(directory, '.cache', file), 'utf8'));
  for (const [options, compared, difference] of [
    [[], 'structure only'],
    [['--positions'], 'positions', '$.end'],
    [['--flags'], 'flags', '$.flags'],
    [['--scanner-hints'], 'scanner-hints', '$.rawText'],
    [['--scanner-hints', '--flags', '--positions'], 'positions, flags, scanner-hints', '$.end'],
  ]) {
    const authored = runner(directory, 'conformance', options, environment);
    assert.equal(authored.status, difference ? 1 : 0, authored.stderr);
    assert.equal(authored.stdout.trim().split('\n').at(-1),
      `${difference ? 0 : 1}/1 structural comparisons passed (TypeScript 6.0.3; compared: ${compared})`);
    if (difference) assert.match(authored.stdout, new RegExp(`AST FAIL case: {"path":"\\${difference}"`));
    for (const [name, args, file] of [
      ['source-corpus', [sources], 'source-corpus.json'],
      ['upstream-corpus', [checkout], 'upstream-corpus.json'],
    ]) {
      const result = runner(directory, name, [...options, ...args], environment);
      assert.equal(result.status, difference ? 1 : 0, result.stderr);
      const saved = report(file);
      assert.equal(saved.compared, compared, name);
      assert.deepEqual(saved.failures.map(failure => failure.path), difference ? [difference] : [], name);
      assert.equal(JSON.parse(result.stdout.trim().split('\n').at(-1)).compared, compared, name);
    }
  }
  // Upstream --check-only compares acceptance, not trees, whatever else is given.
  const checked = runner(directory, 'upstream-corpus', [checkout, '--check-only', '--positions'], environment);
  assert.equal(checked.status, 0, checked.stderr);
  assert.equal(report('upstream-corpus.json').compared, 'acceptance only');
  // Every generated combination is compared with each option.
  const grammar = runner(directory, 'grammar-corpus', COMPARISON_OPTIONS, environment);
  assert.equal(grammar.status, 1, grammar.stderr);
  const generated = report('grammar-corpus.json');
  assert.equal(generated.compared, 'positions, flags, scanner-hints');
  assert.equal(JSON.parse(grammar.stdout.trim().split('\n').at(-1)).compared, 'positions, flags, scanner-hints');
  assert.ok(generated.cases > 0);
  assert.ok(generated.failures.every(failure => failure.path === '$.end'));
  assert.equal(generated.failures.length, generated.cases);
});

test('synthetic authored fixtures fail rather than skip TS7-removed syntax', t => {
  const directory = temporary(t);
  fs.mkdirSync(path.join(directory, 'fixtures'));
  fs.writeFileSync(path.join(directory, 'fixtures/conformance.json'), JSON.stringify([{ name: 'removed', source: '/*unquoted-module*/' }]));
  const result = runner(directory, 'conformance', [], { TYPE_SCRIPT_API: syntheticApi(directory) });
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stdout, /INVALID FIXTURE removed: unquoted module/);
});

test('synthetic upstream native setup failures cannot produce passing reports', t => {
  const directory = temporary(t);
  const checkout = fixture(directory, 'ok.ts', 'const x = 1;');
  const environment = { TYPE_SCRIPT_API: syntheticApi(directory), TSPARSE: executable(directory, 'parser', 'exit 0') };
  for (const body of [
    'if [ "$1" = "--version" ]; then exit 2; fi; exit 0',
    'if [ "$1" = "--version" ]; then echo "Version 7.0.2"; exit 0; fi; echo "error TS5023: Unknown option"; exit 2',
    'if [ "$1" = "--version" ]; then echo "Version 7.0.2"; exit 0; fi; echo "outside.ts(1,1): error TS1000: Syntax error"; exit 1',
  ]) {
    const native = executable(directory, 'native', body);
    const result = runner(directory, 'upstream-native-corpus', [checkout], { ...environment, TYPE_SCRIPT_NATIVE: native });
    assert.notEqual(result.status, 0, result.stdout + result.stderr);
    assert.equal(fs.existsSync(path.join(directory, '.cache/upstream-native.json')), false);
  }
});

test('synthetic grammar invalid cases still fail on Rust infrastructure errors', t => {
  const directory = temporary(t);
  const environment = { TYPE_SCRIPT_API: syntheticApi(directory, { allInvalid: true }), TSPARSE: executable(directory, 'parser', 'exit 2') };
  const result = runner(directory, 'grammar-corpus', [], environment);
  assert.equal(result.status, 1, result.stderr);
  const report = JSON.parse(fs.readFileSync(path.join(directory, '.cache/grammar-corpus.json'), 'utf8'));
  assert.equal(report.invalid, report.cases);
  assert.equal(report.passed, 0);
  assert.equal(report.failures.length, report.cases);
  assert.ok(report.failures.every(failure => failure.status === 2));
});

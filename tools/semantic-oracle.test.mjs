import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import {
  normalizeOracle, clusterDeclarations, classifySymbol, merges, lineStarts, position, loadSemanticApi, semanticCompilerVersion,
  SemanticSession, resultGroups, vendoredLibrary,
} from './semantic-oracle.mjs';
import { VENDORED } from './vendor-typescript-libs.mjs';
import {
  parseMarkers, collectMarkers, checkMarkers, compareResponse, compareGroups, runBatch, nameQueries, compareNameResponse,
} from './semantic-conformance.mjs';

// Real-oracle tests are skipped, with this reason, unless both compilers are configured.
const oracleUnavailable = (!process.env.TYPE_SCRIPT_SEMANTIC_API || !process.env.TYPE_SCRIPT_NATIVE)
  && 'TYPE_SCRIPT_SEMANTIC_API and TYPE_SCRIPT_NATIVE are not both set';
const conformance = fileURLToPath(new URL('./semantic-conformance.mjs', import.meta.url));
const fixtures = fileURLToPath(new URL('../fixtures/references/', import.meta.url));
const exceptions = fileURLToPath(new URL('../fixtures/reference-exceptions/', import.meta.url));

function temporary(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-semantic-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  return directory;
}

function executable(directory, name, script) {
  const file = path.join(directory, name);
  fs.writeFileSync(file, `#!${process.execPath}\n${script}\n`, { mode: 0o755 });
  return file;
}

// Synthetic raw records in the shape `SemanticSession.collect` returns.
const at = (file, line, column, length) => ({ file, start: { line, column }, end: { line, column: column + length } });
const declaration = (name, category, file, line, column, extra = {}) =>
  ({ name, category, kind: 'Synthetic', location: at(file, line, column, name.length), ...extra });
const token = (name, file, line, column, symbol, meaning, extra = {}) =>
  ({ location: at(file, line, column, name.length), name, symbol, meaning, roles: ['use'], ...extra });
const raw = (symbols, tokens) => ({ compiler: '7.0.2', project: '/p/tsconfig.json', files: [], symbols, tokens, diagnostics: [] });

test('merges-with relation covers exactly the documented declaration pairs', () => {
  for (const [a, b] of [['function', 'function'], ['interface', 'interface'], ['class', 'interface'], ['namespace', 'namespace'],
    ['namespace', 'class'], ['function', 'namespace'], ['enum', 'namespace'], ['enum', 'enum'], ['var', 'var'], ['parameter', 'var'],
    ['class', 'function'], ['typeParameter', 'typeParameter'],
    ['module', 'module'], ['module', 'namespace']])
    assert.ok(merges(a, b) && merges(b, a), `${a}+${b}`);
  for (const [a, b] of [['interface', 'var'], ['type', 'const'], ['namespace', 'const'], ['let', 'let'], ['class', 'class'],
    ['function', 'var'], ['parameter', 'let'], ['type', 'interface'], ['interface', 'namespace'], ['export', 'function']])
    assert.ok(!merges(a, b) && !merges(b, a), `${a}+${b}`);
});

test('clusters split same-named declarations by validity and carry their meanings', () => {
  const clusters = symbol => clusterDeclarations(symbol).map(cluster => [cluster.declarations.map(d => d.category).join('+'), cluster.meaning]);
  const at1 = (category, line) => declaration('X', category, 'a.ts', line, 1);
  assert.deepEqual(clusters({ flags: [], declarations: [at1('interface', 1), at1('var', 2)] }), [['interface', 2], ['var', 1]]);
  assert.deepEqual(clusters({ flags: [], declarations: [at1('class', 1), at1('interface', 2), at1('namespace', 3)] }), [['class+interface+namespace', 7]]);
  assert.deepEqual(clusters({ flags: ['ValueModule'], declarations: [at1('function', 1), at1('function', 2), at1('namespace', 3)] }), [['function+function+namespace', 5]]);
  assert.deepEqual(clusters({ flags: [], declarations: [at1('namespace', 1), at1('const', 2)] }), [['namespace', 4], ['const', 1]]);
  assert.deepEqual(clusters({ flags: ['ValueModule'], declarations: [at1('namespace', 1), at1('namespace', 2)] }), [['namespace+namespace', 5]]);
  assert.deepEqual(clusters({ flags: [], declarations: [at1('parameter', 1), at1('var', 2)] }), [['parameter+var', 1]]);
  assert.deepEqual(clusters({ flags: [], declarations: [at1('enum', 1), at1('enum', 2)] }), [['enum+enum', 7]]);
  assert.deepEqual(clusters({ flags: [], declarations: [at1('type', 1), at1('const', 2)] }), [['type', 2], ['const', 1]]);
  assert.deepEqual(clusters({ flags: [], declarations: [at1('member', 1)] }), []);
  // Connected components are transitive: interface-class-namespace-function would need two edges.
  assert.deepEqual(clusters({ flags: [], declarations: [at1('interface', 1), at1('namespace', 2), at1('class', 3)] }), [['interface+namespace+class', 7]]);
});

test('symbol classification separates members and unresolved placeholders from declared targets', () => {
  const symbol = (flags, declarations, extra = {}) => ({ name: 'x', flags, declarations, ...extra });
  const lexical = declaration('x', 'const', 'a.ts', 1, 1);
  assert.equal(classifySymbol(null), 'none');
  assert.equal(classifySymbol(symbol(['BlockScopedVariable'], [lexical])), 'symbol');
  assert.equal(classifySymbol(symbol(['Property'], [declaration('x', 'member', 'a.ts', 1, 1)])), 'member');
  assert.equal(classifySymbol(symbol(['Method'], [])), 'member');
  assert.equal(classifySymbol(symbol(['Property', 'Transient'], [], { special: 'undefined' })), 'member');
  assert.equal(classifySymbol(symbol(['Property', 'Transient'], [], { special: 'arguments' })), 'member');
  assert.equal(classifySymbol(symbol(['Property', 'Transient'], [], { special: 'unknown' })), 'none');
  assert.equal(classifySymbol(symbol(['TypeAlias', 'Transient'], [])), 'none');
  assert.equal(classifySymbol({ name: 'globalThis', flags: ['ValueModule'], declarations: [] }), 'member');
  // A standard library declares targets like any other file.
  assert.equal(classifySymbol(symbol(['Interface'], [{ ...lexical, category: 'interface', defaultLibrary: true }])), 'symbol');
  assert.equal(classifySymbol(symbol(['Interface'], [{ ...lexical, category: 'interface', defaultLibrary: true }, lexical])), 'symbol');
  // `export default <expression>` and `export = <expression>` are Property symbols in TypeScript.
  assert.equal(classifySymbol(symbol(['Property'], [declaration('default', 'export', 'a.ts', 1, 8)])), 'symbol');
});

test('normalization routes each token to the clusters its meaning selects', () => {
  const box = { name: 'Box', flags: ['FunctionScopedVariable', 'Interface'], declarations: [
    declaration('Box', 'interface', 'a.ts', 1, 11), declaration('Box', 'var', 'a.ts', 2, 13)] };
  const oracle = normalizeOracle(raw([box], [
    token('Box', 'a.ts', 1, 11, 0, ['value', 'type', 'namespace'], { declares: box.declarations[0], roles: ['declaration'] }),
    token('Box', 'a.ts', 2, 13, 0, ['value', 'type', 'namespace'], { declares: box.declarations[1], roles: ['declaration'] }),
    token('Box', 'a.ts', 3, 8, 0, ['type']),
    token('Box', 'a.ts', 3, 20, 0, ['value']),
    token('Box', 'a.ts', 4, 10, 0, ['value', 'type', 'namespace'], { roles: ['export'] }),
  ]));
  assert.deepEqual(oracle.tokens.map(t => t.groups), [[0], [1], [0], [1], [0, 1]]);
  assert.deepEqual(oracle.tokens.map(t => t.outcome), ['groups', 'groups', 'groups', 'groups', 'groups']);
  assert.deepEqual(oracle.groups.map(g => [g.meaning, g.declarations.map(d => d.location.start.line), g.occurrences.length]),
    [[['type'], [1], 3], [['value'], [2], 3]]);
  assert.deepEqual(resultGroups(oracle, [1]), [{ declarations: [{ name: 'Box', location: at('a.ts', 2, 13, 3) }],
    occurrences: [[at('a.ts', 2, 13, 3), ['declaration']], [at('a.ts', 3, 20, 3), ['use']], [at('a.ts', 4, 10, 3), ['export']]]
      .map(([location, roles]) => ({ name: 'Box', location, roles })) }]);
  assert.equal(oracle.anomalies.length, 0);
});

test('distinct TypeScript symbols with the same declarations form one group', () => {
  const module = declaration('api.ts', 'module', 'api.ts', 1, 1);
  module.location = { file: 'api.ts', start: { line: 1, column: 1 }, end: { line: 1, column: 1 } };
  const plain = { name: '"api"', flags: ['ValueModule'], declarations: [module] };
  const transient = { name: '"api"', flags: ['ValueModule', 'Transient'], declarations: [{ ...module }] };
  const oracle = normalizeOracle(raw([plain, transient], [
    token('whole', 'b.ts', 1, 13, 1, ['value', 'type', 'namespace'], { roles: ['import'] }),
    token('ns', 'b.ts', 2, 10, 0, ['value', 'type', 'namespace'], { roles: ['import'] }),
    token('whole', 'b.ts', 3, 1, 1, ['value']),
  ]));
  assert.equal(oracle.groups.length, 1);
  assert.deepEqual(oracle.tokens.map(t => t.groups), [[0], [0], [0]]);
  assert.deepEqual(oracle.groups[0].meaning, ['value', 'namespace']);
});

test('a declaration name keeps its own cluster even when TypeScript returns another symbol', () => {
  const parameter = declaration('x', 'parameter', 'a.ts', 1, 21);
  const property = { name: 'x', flags: ['Property'], declarations: [parameter] };
  const lexical = { name: 'x', flags: ['FunctionScopedVariable'], declarations: [{ ...parameter }] };
  const oracle = normalizeOracle(raw([property, lexical], [
    token('x', 'a.ts', 1, 21, 0, ['value', 'type', 'namespace'], { declares: parameter, roles: ['declaration'] }),
    token('x', 'a.ts', 1, 40, 1, ['value']),
    token('x', 'a.ts', 1, 50, 0, ['value'], { syntax: 'property' }),
  ]));
  assert.deepEqual(oracle.tokens.map(t => [t.classification, t.groups, t.outcome]),
    [['symbol', [0], 'groups'], ['symbol', [0], 'groups'], ['member', [], 'unsupported']]);
  assert.equal(oracle.tokens[0].note, 'declaration outside the TypeScript target symbol');
  assert.deepEqual(oracle.tokens.map(t => t.normalization), ['declarationOutsideTarget', undefined, undefined]);
});

test('unmatched meanings become anomalies instead of silently dropped occurrences', () => {
  const only = { name: 'T', flags: ['TypeAlias'], declarations: [declaration('T', 'type', 'a.ts', 1, 6)] };
  const oracle = normalizeOracle(raw([only], [token('T', 'a.ts', 2, 1, 0, ['value'])]));
  assert.deepEqual(oracle.tokens[0].groups, []);
  assert.equal(oracle.tokens[0].outcome, 'anomaly');
  assert.equal(oracle.anomalies.length, 1);
  assert.match(oracle.anomalies[0].reason, /no declaration cluster/);
});

test('non-reference syntax expects UnsupportedTarget; other unresolved tokens expect UnresolvedTarget', () => {
  const library = { name: 'Array', flags: ['Interface'], declarations: [declaration('Array', 'interface', 'lib.d.ts', 1, 11, { defaultLibrary: true })] };
  const global = { name: '__global', flags: ['NamespaceModule'], declarations: [declaration('global', 'namespace', 'a.ts', 1, 9)] };
  const process = { name: 'process', flags: ['BlockScopedVariable'], declarations: [declaration('process', 'const', 'node.d.ts', 1, 9)] };
  const oracle = normalizeOracle(raw([library, global, process], [
    token('outer', 'a.ts', 1, 1, null, [], { syntax: 'label' }),
    token('div', 'a.tsx', 1, 2, null, [], { syntax: 'jsxIntrinsic' }),
    token('thing', 'a.ts', 2, 9, null, ['value'], { syntax: 'property' }),
    token('meta', 'a.ts', 3, 8, null, [], { syntax: 'metaProperty' }),
    token('missing', 'a.ts', 4, 1, null, ['value']),
    token('Array', 'a.ts', 5, 8, 0, ['type']),
    token('global', 'a.ts', 6, 9, 1, [], { syntax: 'globalAugmentation' }),
    // `const { default: process } = await import("node:process")`: TypeScript
    // resolves the property key to the export; the design treats it as a key.
    token('default', 'a.ts', 7, 9, 2, [], { syntax: 'propertyName' }),
    token('process', 'a.ts', 8, 1, 2, ['value'], { syntax: 'property' }),
  ]));
  assert.deepEqual(oracle.tokens.map(t => [t.classification, t.outcome]), [['none', 'unsupported'], ['none', 'unsupported'],
    ['none', 'unsupported'], ['none', 'unsupported'], ['none', 'unresolved'], ['symbol', 'groups'], ['symbol', 'unsupported'],
    ['symbol', 'unsupported'], ['symbol', 'groups']]);
  assert.match(oracle.tokens[7].note, /non-reference name to `process`/);
  // A library-declared target has a real group with its library declaration.
  assert.deepEqual(oracle.groups.map(g => [g.declarations.map(d => d.location.file), g.occurrences.map(o => o.name)]),
    [[['lib.d.ts'], ['Array']], [['node.d.ts'], ['process']]]);
  assert.deepEqual(oracle.anomalies, []);
  // Only departures from what TypeScript resolves are normalizations.
  assert.deepEqual(oracle.tokens.map(t => t.normalization), [undefined, undefined, 'unresolvedProperty', undefined, undefined,
    undefined, 'nonReference:globalAugmentation', 'nonReference:propertyName', undefined]);
});

test('missing namespace exports and value uses of type-only aliases expect UnresolvedTarget', () => {
  const shape = { name: 'Shape', flags: ['Interface', 'BlockScopedVariable'], declarations: [
    declaration('Shape', 'interface', 'lib.ts', 1, 18), declaration('Shape', 'const', 'lib.ts', 2, 14)] };
  const oracle = normalizeOracle(raw([shape], [
    token('missing', 'a.ts', 1, 4, null, ['value'], { syntax: 'property', namespaceQualifier: true }),
    token('length', 'a.ts', 2, 3, null, ['value'], { syntax: 'property' }),
    token('Shape', 'a.ts', 3, 1, 0, ['value'], { typeOnlyValue: true }),
    token('Shape', 'a.ts', 4, 1, 0, ['type']),
  ]));
  assert.deepEqual(oracle.tokens.map(t => t.outcome), ['unresolved', 'unsupported', 'unresolved', 'groups']);
  assert.deepEqual(oracle.tokens.map(t => t.normalization), ['missingNamespaceExport', 'unresolvedProperty', 'typeOnlyValue', undefined]);
});

test('built-in targets and receiver-dependent properties carry normalization codes', () => {
  const value = { name: 'box', flags: ['BlockScopedVariable'], declarations: [declaration('box', 'const', 'a.ts', 1, 7)] };
  const size = { name: 'size', flags: ['BlockScopedVariable'], declarations: [declaration('size', 'const', 'a.ts', 2, 7)] };
  const undefinedSymbol = { name: 'undefined', flags: ['Property', 'Transient'], declarations: [], special: 'undefined' };
  const globalThisSymbol = { name: 'globalThis', flags: ['ValueModule'], declarations: [] };
  const oracle = normalizeOracle(raw([value, size, undefinedSymbol, globalThisSymbol], [
    token('undefined', 'a.ts', 3, 1, 2, ['value']),
    token('globalThis', 'a.ts', 4, 1, 3, ['value']),
    // `box.size` resolves through the type of `box`; `gone.size` has no receiver.
    token('box', 'a.ts', 5, 1, 0, ['value']),
    token('size', 'a.ts', 5, 5, 1, ['value'], { syntax: 'property', receiver: at('a.ts', 5, 1, 3) }),
    token('gone', 'a.ts', 6, 1, null, ['value']),
    token('size', 'a.ts', 6, 6, 1, ['value'], { syntax: 'property', receiver: at('a.ts', 6, 1, 4) }),
  ]));
  assert.deepEqual(oracle.tokens.map(t => [t.outcome, t.normalization]), [['unsupported', 'builtin'], ['unsupported', 'builtin'],
    ['groups', undefined], ['unsupported', 'receiverType'], ['unresolved', undefined], ['unsupported', 'unresolvedReceiver']]);
});

test('groups are ordered by their earliest declaration and occurrences by location', () => {
  const late = { name: 'b', flags: ['BlockScopedVariable'], declarations: [declaration('b', 'const', 'z.ts', 1, 7)] };
  const early = { name: 'a', flags: ['BlockScopedVariable'], declarations: [declaration('a', 'const', 'a.ts', 9, 7)] };
  const oracle = normalizeOracle(raw([late, early], [
    token('b', 'z.ts', 1, 7, 0, ['value']), token('a', 'b.ts', 1, 1, 1, ['value']), token('a', 'a.ts', 9, 7, 1, ['value']),
  ]));
  assert.deepEqual(oracle.groups.map(g => g.declarations[0].name), ['a', 'b']);
  assert.deepEqual(oracle.groups[0].occurrences.map(o => o.location.file), ['a.ts', 'b.ts']);
  assert.deepEqual(oracle.tokens.map(t => t.groups), [[1], [0], [0]]);
});

test('locations use one-based lines and UTF-16 columns for every TypeScript line break', () => {
  const text = 'a\r\nb\rc d e\nf\u{1F600}g';
  const starts = lineStarts(text);
  assert.deepEqual(starts, [0, 3, 5, 7, 9, 11]);
  assert.deepEqual(position(starts, 0), { line: 1, column: 1 });
  assert.deepEqual(position(starts, 2), { line: 1, column: 3 });
  assert.deepEqual(position(starts, 9), { line: 5, column: 1 });
  assert.deepEqual(position(starts, text.indexOf('g')), { line: 6, column: 4 });
});

test('markers follow the Rust harness syntax, including escapes, astral names, and roles', () => {
  const text = 'const /*@a:de*/x = 1;\r\n/*@a,b:u*/x; /*@!*/\\u0061b; /*@?*/\\u{1D465}y;\u2028/*@c*/\u{1D465}_$ + 1;';
  const markers = parseMarkers(text, 'f.ts');
  assert.deepEqual(markers.map(m => [m.name, m.location.start, m.location.end]), [
    ['x', { line: 1, column: 16 }, { line: 1, column: 17 }],
    ['x', { line: 2, column: 11 }, { line: 2, column: 12 }],
    ['ab', { line: 2, column: 20 }, { line: 2, column: 27 }],
    ['\u{1D465}y', { line: 2, column: 35 }, { line: 2, column: 45 }],
    ['\u{1D465}_$', { line: 3, column: 7 }, { line: 3, column: 11 }],
  ]);
  assert.deepEqual(markers.map(m => m.expect), [
    { kind: 'groups', labels: ['a'], roles: ['declaration', 'export'] },
    { kind: 'groups', labels: ['a', 'b'], roles: ['use'] },
    { kind: 'unsupported' }, { kind: 'unresolved' },
    { kind: 'groups', labels: ['c'], roles: null },
  ]);
  assert.deepEqual(parseMarkers('class C { /*@!*/#count = 0 }', 'f.ts').map(m => [m.name, m.location.start.column, m.location.end.column]),
    [['#count', 17, 23]]);
  assert.throws(() => parseMarkers('/*@a*/ x', 'f.ts'), /precedes no identifier/);
  assert.throws(() => parseMarkers('/*@a:z*/x', 'f.ts'), /unknown role z/);
  assert.throws(() => parseMarkers('/*@a', 'f.ts'), /unclosed marker/);
  assert.throws(() => parseMarkers('/*@a,*/x', 'f.ts'), /empty marker label/);
});

test('marker collection walks TypeScript sources and reports paths relative to the fixture', t => {
  const directory = temporary(t);
  fs.mkdirSync(path.join(directory, 'sub'));
  fs.writeFileSync(path.join(directory, 'sub/b.tsx'), '/*@b*/y');
  fs.writeFileSync(path.join(directory, 'a.ts'), '/*@a*/x');
  fs.writeFileSync(path.join(directory, 'notes.md'), '/*@n*/ignored');
  assert.deepEqual(collectMarkers(directory).map(m => `${m.location.file}:${m.name}`), ['a.ts:x', 'sub/b.tsx:y']);
});

// A small normalized oracle: `run` (two tokens), a parameter, and a label,
// one per line at column 13 so that marker text can line up with them.
const SAMPLE_NAMES = ['run', 's', 'run', 'lbl'];
function sampleOracle() {
  const run = { name: 'run', flags: ['Function'], declarations: [declaration('run', 'function', 'a.ts', 1, 13)] };
  const param = { name: 's', flags: ['FunctionScopedVariable'], declarations: [declaration('s', 'parameter', 'a.ts', 2, 13)] };
  return normalizeOracle(raw([run, param], [
    token('run', 'a.ts', 1, 13, 0, ['value', 'type', 'namespace'], { declares: run.declarations[0], roles: ['declaration'], candidate: true }),
    token('s', 'a.ts', 2, 13, 1, ['value', 'type', 'namespace'], { declares: param.declarations[0], roles: ['declaration'], candidate: true }),
    token('run', 'a.ts', 3, 13, 0, ['value']),
    token('lbl', 'a.ts', 4, 13, null, [], { syntax: 'label', roles: [] }),
  ]));
}
const sampleText = (specs, extra = '') =>
  specs.map((spec, index) => `${' '.repeat(12 - (spec ?? '').length)}${spec ?? ''}${SAMPLE_NAMES[index]}`).join('\n') + extra;

test('marker checks accept agreeing markers and name each kind of disagreement', () => {
  const oracle = sampleOracle();
  const kinds = (specs, extra) => [...new Set(checkMarkers(oracle, parseMarkers(sampleText(specs, extra), 'a.ts')).map(p => p.kind))].sort();
  assert.deepEqual(checkMarkers(oracle, parseMarkers(sampleText(['/*@r:d*/', '/*@p*/', '/*@r:u*/', '/*@!*/']), 'a.ts')), []);
  assert.deepEqual(kinds(['/*@r*/', '/*@p*/', '/*@p*/', '/*@!*/']), ['label']);
  assert.deepEqual(kinds(['/*@r*/', '/*@r*/', '/*@r*/', '/*@!*/']), ['label']);
  assert.deepEqual(kinds(['/*@r*/', '/*@p*/', '/*@r,p*/', '/*@!*/']), ['label']);
  assert.deepEqual(kinds(['/*@r*/', '/*@p*/', '/*@r*/', '/*@?*/']), ['classification']);
  assert.deepEqual(kinds(['/*@r:e*/', '/*@p*/', '/*@r*/', '/*@!*/']), ['roles']);
  assert.deepEqual(kinds(['/*@r*/', '/*@p*/', null, '/*@!*/']), ['label', 'unmarked']);
  assert.deepEqual(kinds(['/*@r*/', '/*@p*/', '/*@r*/', '/*@!*/'], '\n/*@x*/extra'), ['label', 'no-token']);
  const anomalous = normalizeOracle(raw([{ name: 'T', flags: ['TypeAlias'], declarations: [declaration('T', 'type', 'a.ts', 1, 6)] }],
    [token('T', 'a.ts', 1, 7, 0, ['value'])]));
  assert.deepEqual(checkMarkers(anomalous, parseMarkers('/*@t*/T', 'a.ts')).map(p => p.kind).sort(), ['anomaly', 'classification', 'label']);
});

test('tool comparison classifies every difference without discarding locations', () => {
  const oracle = sampleOracle();
  const group = index => resultGroups(oracle, [index])[0];
  const result = groups => ({ result: { project: '/p/tsconfig.json', groups } });
  const kinds = (token, response) => compareResponse(oracle, oracle.tokens[token], response).map(d => d.kind).sort();
  assert.deepEqual(kinds(0, result([group(0)])), []);
  assert.deepEqual(kinds(3, { error: { kind: 'UnsupportedTarget', message: 'label' } }), []);
  assert.deepEqual(kinds(3, { error: { kind: 'UnresolvedTarget', message: 'label' } }), ['classification']);
  assert.deepEqual(kinds(0, { error: { kind: 'UnsupportedTarget', message: 'no' } }), ['classification']);
  assert.deepEqual(kinds(0, result([])), ['missing-group']);
  assert.deepEqual(kinds(0, { panic: 'boom' }), ['panic']);
  assert.deepEqual(kinds(0, { unexpected: true }), ['invalid-response']);
  const missing = { ...group(0), occurrences: group(0).occurrences.slice(0, 1) };
  assert.deepEqual(kinds(0, result([missing])), ['missing-occurrence']);
  const extra = { ...group(0), occurrences: [...group(0).occurrences, { name: 's', location: at('a.ts', 9, 9, 1) }] };
  assert.deepEqual(kinds(0, result([extra])), ['extra-occurrence']);
  const merged = { declarations: [...group(0).declarations, ...group(1).declarations], occurrences: [...group(0).occurrences, ...group(1).occurrences] };
  assert.deepEqual(kinds(0, result([merged])), ['extra-declaration', 'extra-occurrence']);
  assert.deepEqual(compareGroups([group(0), group(1)], [merged]).map(d => d.kind).sort(), ['extra-declaration', 'extra-declaration', 'merged-groups']);
  const [first, second] = group(0).occurrences;
  const split = [{ declarations: group(0).declarations, occurrences: [first] }, { declarations: [], occurrences: [second] }];
  assert.deepEqual(kinds(0, result(split)), ['split-group']);
  const moved = { declarations: [{ name: 'run', location: at('a.ts', 7, 1, 3) }], occurrences: group(0).occurrences };
  assert.deepEqual(kinds(0, result([moved])), ['extra-declaration', 'missing-declaration']);
  const renamed = { ...group(0), occurrences: group(0).occurrences.map(o => ({ ...o, name: 'other' })) };
  assert.deepEqual(kinds(0, result([renamed])), ['occurrence-name']);
  // Roles are compared as sets on every matched occurrence.
  const withRoles = roles => ({ ...group(0), occurrences: group(0).occurrences.map((o, index) => ({ ...o, roles: roles(o.roles, index) })) });
  assert.deepEqual(kinds(0, result([withRoles(roles => [...roles].reverse())])), []);
  assert.deepEqual(kinds(0, result([withRoles(() => ['use'])])), ['occurrence-roles']);
  assert.deepEqual(kinds(0, result([withRoles((roles, index) => (index ? roles : [...roles, 'export']))])), ['occurrence-roles']);
  assert.deepEqual(kinds(0, result([withRoles(() => undefined)])), ['occurrence-roles']);
  const [reroled] = compareResponse(oracle, oracle.tokens[0], result([withRoles(() => ['use'])]));
  assert.deepEqual([reroled.count, reroled.locations], [1, ['a.ts:1:13 declaration -> use']]);
  assert.deepEqual(kinds(0, result([group(0), group(1)])), ['extra-group']);
  // `export { Box }` belongs to both the type and the value group; sharing it
  // does not make the groups split or merged.
  const shared = { name: 'Box', location: at('a.ts', 9, 10, 3) };
  const typeBox = { declarations: [{ name: 'Box', location: at('a.ts', 1, 11, 3) }], occurrences: [{ name: 'Box', location: at('a.ts', 1, 11, 3) }, shared] };
  const valueBox = { declarations: [{ name: 'Box', location: at('a.ts', 2, 13, 3) }], occurrences: [{ name: 'Box', location: at('a.ts', 2, 13, 3) }, shared] };
  assert.deepEqual(compareGroups([typeBox, valueBox], [valueBox, typeBox]), []);
  assert.deepEqual(compareGroups([typeBox, valueBox], [{ declarations: [...typeBox.declarations, ...valueBox.declarations],
    occurrences: [typeBox.occurrences[0], valueBox.occurrences[0], shared] }]).map(d => d.kind).sort(), ['extra-declaration', 'extra-declaration', 'merged-groups']);
  // A one-way merge, such as a pattern ambient module augmented for one
  // specifier, lists the pattern's declaration in both groups; sharing it does
  // not make the groups split or merged either.
  const pattern = { name: 'Styles', location: at('css.d.ts', 2, 13, 6) };
  const plain = { declarations: [pattern], occurrences: [{ name: 'Styles', location: at('y.ts', 1, 15, 6) }] };
  const augmented = { declarations: [pattern, { name: 'Styles', location: at('x.ts', 3, 13, 6) }],
    occurrences: [{ name: 'Styles', location: at('x.ts', 1, 15, 6) }, { name: 'Styles', location: at('x.ts', 3, 13, 6) }] };
  assert.deepEqual(compareGroups([augmented, plain], [plain, augmented]), []);
  assert.deepEqual(compareGroups([augmented, plain], [{ declarations: augmented.declarations,
    occurrences: [...augmented.occurrences, ...plain.occurrences] }]).map(d => d.kind).sort(), ['extra-declaration', 'merged-groups']);
  const many = { declarations: group(0).declarations, occurrences: Array.from({ length: 30 }, (_, line) => ({ name: 'run', location: at('b.ts', line + 1, 1, 3) })) };
  const [difference] = compareGroups([many], [{ declarations: group(0).declarations, occurrences: [] }]);
  assert.equal(difference.kind, 'missing-occurrence');
  assert.equal(difference.count, 30);
  assert.equal(difference.locations.length, 20);
  const anomalous = normalizeOracle(raw([{ name: 'T', flags: ['TypeAlias'], declarations: [declaration('T', 'type', 'a.ts', 1, 6)] }],
    [token('T', 'a.ts', 1, 1, 0, ['value'])]));
  assert.deepEqual(compareResponse(anomalous, anomalous.tokens[0], result([])).map(d => d.kind), ['oracle-anomaly']);
});

test('group comparison reports repeated occurrences and declarations', () => {
  const location = (line, column) => ({ file: 'a.ts', start: { line, column }, end: { line, column: column + 1 } });
  const expected = [{ declarations: [{ name: 'x', location: location(1, 7) }],
    occurrences: [{ name: 'x', location: location(1, 7) }, { name: 'x', location: location(2, 1) }] }];
  const actual = structuredClone(expected);
  actual[0].occurrences.push(structuredClone(actual[0].occurrences[1]));
  actual[0].declarations.push(structuredClone(actual[0].declarations[0]));
  assert.deepEqual(compareGroups(expected, actual).map(difference => difference.kind), ['duplicate-occurrence', 'duplicate-declaration']);
  assert.deepEqual(compareGroups(expected, structuredClone(expected)), []);
});

test('name queries expect the union of their candidates groups or a failing candidate error', () => {
  const result = groups => ({ result: { project: '/p/tsconfig.json', groups } });
  const error = (kind, line, column) => ({ error: { kind, message: kind, path: '/p/a.ts', position: { line, column } } });
  const sample = sampleOracle();
  // Uses and labels are not candidates; one query per distinct spelling.
  assert.deepEqual(nameQueries(sample).map(query => [query.name, query.candidates.map(t => t.location.start.line)]), [['run', [1]], ['s', [2]]]);
  assert.deepEqual(nameQueries(sample, sample.tokens.slice(1)).map(query => query.name), ['s']);
  const [run] = nameQueries(sample);
  const kinds = (oracle, query, response) => compareNameResponse(oracle, query, response).map(d => d.kind).sort();
  assert.deepEqual(kinds(sample, run, result(resultGroups(sample, [0]))), []);
  assert.deepEqual(kinds(sample, run, result([])), ['missing-group']);
  assert.deepEqual(kinds(sample, run, error('UnresolvedTarget', 1, 13)), ['classification']);
  assert.deepEqual(kinds(sample, run, { panic: 'boom' }), ['panic']);

  // `x` is declared twice (two groups) and imported from a missing module
  // (unresolved); `y` is imported both from a missing module and as a member
  // of an `export =` namespace (unsupported).
  const first = { name: 'x', flags: ['BlockScopedVariable'], declarations: [declaration('x', 'const', 'b.ts', 1, 7)] };
  const second = { name: 'x', flags: ['FunctionScopedVariable'], declarations: [declaration('x', 'parameter', 'c.ts', 1, 12)] };
  const member = { name: 'y', flags: ['Method'], declarations: [declaration('y', 'member', 'd.d.ts', 1, 30)] };
  const all = ['value', 'type', 'namespace'];
  const oracle = normalizeOracle(raw([first, second, member], [
    token('x', 'a.ts', 1, 10, null, all, { roles: ['import'], candidate: true }),
    token('x', 'b.ts', 1, 7, 0, all, { declares: first.declarations[0], roles: ['declaration'], candidate: true }),
    token('x', 'c.ts', 1, 12, 1, all, { declares: second.declarations[0], roles: ['declaration'], candidate: true }),
    token('y', 'a.ts', 2, 10, null, all, { roles: ['import'], candidate: true }),
    token('y', 'a.ts', 3, 10, 2, all, { roles: ['import'], candidate: true }),
  ]));
  const [x, y] = nameQueries(oracle);
  assert.deepEqual(kinds(oracle, x, error('UnresolvedTarget', 1, 10)), []);
  assert.deepEqual(kinds(oracle, x, result(resultGroups(oracle))), ['classification']);
  assert.deepEqual(kinds(oracle, x, error('UnsupportedTarget', 1, 10)), ['classification']);
  assert.deepEqual(kinds(oracle, x, error('UnresolvedTarget', 1, 11)), ['error-location']);
  assert.deepEqual(kinds(oracle, x, { error: { kind: 'UnresolvedTarget', message: 'no location' } }), ['error-location']);
  // Either failing candidate may be reported, but only with its own kind.
  assert.deepEqual(kinds(oracle, y, error('UnresolvedTarget', 2, 10)), []);
  assert.deepEqual(kinds(oracle, y, error('UnsupportedTarget', 3, 10)), []);
  assert.deepEqual(kinds(oracle, y, error('UnsupportedTarget', 2, 10)), ['error-location']);
  // Without the unresolved import, the query unites both declarations' groups.
  const resolved = { ...x, candidates: x.candidates.slice(1) };
  assert.deepEqual(kinds(oracle, resolved, result(resultGroups(oracle))), []);
  assert.deepEqual(kinds(oracle, resolved, result(resultGroups(oracle, [1]))), ['missing-group']);
});

test('batch runs stream answers and treat crashes, short answers, and project errors as setup failures', async t => {
  const directory = temporary(t);
  const echo = executable(directory, 'echo-batch', `
    const lines = require('node:fs').readFileSync(0, 'utf8').split('\\n').filter(Boolean);
    for (const line of lines) console.log(JSON.stringify({ error: { kind: 'UnresolvedTarget', message: line } }));`);
  const answers = [];
  const queries = [{ at: { file: 'a.ts', line: 1, column: 1 } }, { name: 'x', file: null }];
  const ok = await runBatch(echo, '/p/tsconfig.json', queries, (index, response) => answers.push([index, response.error.message]));
  assert.equal(ok.error, undefined);
  assert.deepEqual(answers, [[0, JSON.stringify(queries[0])], [1, JSON.stringify(queries[1])]]);
  const short = executable(directory, 'short-batch', `console.log(JSON.stringify({ result: { groups: [] } }));`);
  assert.match((await runBatch(short, '/p', queries, () => {})).error, /answered 1 of 2/);
  const crash = executable(directory, 'crash-batch', `process.stderr.write('bad'); process.exit(3);`);
  assert.match((await runBatch(crash, '/p', queries, () => {})).error, /exit 3.*bad/);
  const project = executable(directory, 'project-batch', `console.log(JSON.stringify({ projectError: { kind: 'Io', message: 'missing' } })); process.exit(1);`);
  const failed = await runBatch(project, '/p', queries, () => assert.fail('no responses expected'));
  assert.deepEqual(failed.projectError, { kind: 'Io', message: 'missing' });
  assert.match((await runBatch(path.join(directory, 'missing'), '/p', queries, () => {})).error, /ENOENT/);
});

test('oracle identity requires the pinned 7.0.2 package and native binary', async t => {
  const directory = temporary(t);
  const native = version => executable(directory, `native-${version}`, `console.log('Version ${version}')`);
  assert.equal(semanticCompilerVersion(native('7.0.2')), 'Version 7.0.2');
  assert.throws(() => semanticCompilerVersion(native('7.0.1')), /7\.0\.2 native compiler/);
  assert.throws(() => semanticCompilerVersion(path.join(directory, 'missing')), /7\.0\.2 native compiler/);
  const failing = executable(directory, 'native-failing', `console.log('Version 7.0.2'); process.exit(2);`);
  assert.throws(() => semanticCompilerVersion(failing), /7\.0\.2 native compiler/);
  const api = path.join(directory, 'api');
  fs.mkdirSync(api);
  fs.writeFileSync(path.join(api, 'package.json'), JSON.stringify({ version: '7.0.1' }));
  await assert.rejects(loadSemanticApi({ api, native: native('7.0.2') }), /Expected semantic API 7\.0\.2, received 7\.0\.1/);
  fs.writeFileSync(path.join(api, 'package.json'), JSON.stringify({ version: '7.0.2' }));
  await assert.rejects(loadSemanticApi({ api, native: native('7.0.1') }), /7\.0\.2 native compiler/);
  await assert.rejects(loadSemanticApi({ api: path.join(directory, 'none'), native: native('7.0.2') }), /no readable package\.json/);
});

test('conformance CLI rejects bad usage and setup before comparing anything', t => {
  const directory = temporary(t);
  const run = (args, environment = {}) => spawnSync(process.execPath, [conformance, ...args],
    { cwd: directory, env: { ...process.env, ...environment }, encoding: 'utf8', timeout: 30_000 });
  for (const [args, message] of [[[], /Usage/], [['verify'], /Usage/], [['markers', '--limit=0'], /positive integer/],
    [['compare', '--strict'], /Unknown option/]]) {
    const result = run(args);
    assert.equal(result.status, 1, result.stderr);
    assert.match(result.stderr, message);
  }
  const result = run(['markers'], { TYPE_SCRIPT_SEMANTIC_API: path.join(directory, 'none'), TYPE_SCRIPT_NATIVE: path.join(directory, 'none') });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /package\.json/);
});

test('real oracle: positions, aliases, shadowing, and markers agree on a fixture', { skip: oracleUnavailable }, async t => {
  const semantic = await loadSemanticApi();
  const session = new SemanticSession(semantic);
  t.after(() => session.close());
  const directory = temporary(t);
  // An astral string before a token on the same line distinguishes UTF-16 from UTF-8 offsets.
  fs.writeFileSync(path.join(directory, 'tsconfig.json'), '{ "compilerOptions": { "noLib": true } }');
  fs.writeFileSync(path.join(directory, 'a.ts'), 'const s = "\u{1F600}"; const \\u0061b = 1; ab;\n');
  const oracle = normalizeOracle(session.collect(path.join(directory, 'tsconfig.json')));
  // UTF-8 byte offsets would place `ab` at column 25, not 23.
  assert.deepEqual(oracle.tokens.map(t => [t.name, t.location.start.column, t.location.end.column]), [['s', 7, 8], ['ab', 23, 30], ['ab', 36, 38]]);
  const fixture = path.join(fixtures, 'aliases-shadowing');
  const aliases = normalizeOracle(session.collect(path.join(fixture, 'tsconfig.json')));
  assert.deepEqual(checkMarkers(aliases, collectMarkers(fixture)), []);
  assert.deepEqual(aliases.groups.map(g => [g.declarations.map(d => `${d.name}@${d.location.file}`).join(), g.occurrences.length]),
    [['run@api.ts', 7], ['copy@api.ts', 2], ['local@app.ts', 1], ['start@app.ts', 2]]);
  // Name candidates: declarations, the local name of an import, and the
  // exported name of a re-export, but not the names they rename.
  assert.deepEqual(aliases.tokens.filter(t => t.candidate).map(t => `${t.location.file}:${t.location.start.line}:${t.name}`).sort(),
    ['api.ts:2:run', 'api.ts:3:copy', 'app.ts:2:start', 'app.ts:4:local', 'app.ts:4:start', 'barrel.ts:1:execute']);
});

test('compiler library files are located at their vendored copies, by file name only', t => {
  const directory = temporary(t);
  const compiler = path.join(directory, 'compiler');
  const vendored = path.join(directory, 'vendored');
  for (const dir of [compiler, vendored, path.join(directory, 'other')]) fs.mkdirSync(dir);
  for (const file of [path.join(compiler, 'lib.es5.d.ts'), path.join(vendored, 'lib.es5.d.ts'), path.join(compiler, 'lib.extra.d.ts'),
    path.join(directory, 'other', 'lib.es5.d.ts')]) fs.writeFileSync(file, '');
  const real = fs.realpathSync(compiler);
  assert.equal(vendoredLibrary(path.join(compiler, 'lib.es5.d.ts'), real, vendored), path.join(vendored, 'lib.es5.d.ts'));
  // A replacement package's library keeps its own path.
  assert.equal(vendoredLibrary(path.join(directory, 'other', 'lib.es5.d.ts'), real, vendored), undefined);
  assert.throws(() => vendoredLibrary(path.join(compiler, 'lib.extra.d.ts'), real, vendored), /no vendored copy/);
});

test('real oracle: library targets have groups whose library declarations are vendored', { skip: oracleUnavailable }, async t => {
  const session = new SemanticSession(await loadSemanticApi());
  t.after(() => session.close());
  const directory = temporary(t);
  fs.writeFileSync(path.join(directory, 'tsconfig.json'), '{ "compilerOptions": { "lib": ["es2015.promise", "es5"] } }');
  fs.writeFileSync(path.join(directory, 'a.ts'), 'const p: Promise<number> = Promise.resolve(1);\n');
  const oracle = normalizeOracle(session.collect(path.join(directory, 'tsconfig.json')));
  assert.deepEqual(oracle.anomalies, []);
  // Library files are not searched: every token is in a.ts.
  assert.deepEqual([...new Set(oracle.tokens.map(t => t.location.file))], ['a.ts']);
  const root = fs.realpathSync(directory);
  const library = name => path.relative(root, path.join(VENDORED, name)).split(path.sep).join('/');
  const promise = oracle.tokens.filter(t => t.name === 'Promise');
  assert.deepEqual(promise.map(t => [t.outcome, t.normalization]), [['groups', undefined], ['groups', undefined]]);
  assert.deepEqual(promise.map(t => oracle.groups[t.groups[0]].declarations.map(d => `${d.location.file}:${d.location.start.line}`)),
    [[`${library('lib.es5.d.ts')}:1548`], [`${library('lib.es2015.promise.d.ts')}:79`]]);
});

test('real oracle: mixed aliases retain independent type and value declarations', { skip: oracleUnavailable }, async t => {
  const session = new SemanticSession(await loadSemanticApi());
  t.after(() => session.close());
  const fixture = path.join(fixtures, 'mixed-alias-meanings');
  const oracle = normalizeOracle(session.collect(path.join(fixture, 'tsconfig.json')));
  assert.deepEqual(oracle.anomalies, []);
  assert.deepEqual(checkMarkers(oracle, collectMarkers(fixture)), []);
});

test('real oracle: ambiguous star exports are unresolved where TypeScript keeps the first target', { skip: oracleUnavailable }, async t => {
  const session = new SemanticSession(await loadSemanticApi());
  t.after(() => session.close());
  const fixture = path.join(fixtures, 'star-exports');
  const oracle = normalizeOracle(session.collect(path.join(fixture, 'tsconfig.json')));
  assert.deepEqual(oracle.anomalies, []);
  assert.deepEqual(checkMarkers(oracle, collectMarkers(fixture)), []);
  // TypeScript reports TS2308 at the second `export *` and resolves these
  // tokens to left.ts: through an import, its uses, a namespace member, and
  // both names of a re-export.
  assert.ok(oracle.diagnostics.some(diagnostic => diagnostic.code === 2308 && diagnostic.file === 'both.ts'));
  assert.deepEqual(oracle.tokens.filter(t => t.normalization === 'ambiguousStarExport')
    .map(t => `${t.location.file}:${t.location.start.line}:${t.name}:${t.outcome}`).sort(),
  ['app.ts:10:shared:unresolved', 'app.ts:11:shared:unresolved', 'app.ts:13:forwarded:unresolved', 'app.ts:13:shared:unresolved',
    'app.ts:3:shared:unresolved']);
});

// The compiler's answer at `file:line:column` in the shape an exception
// fixture's `expected.json` records: groups of `file:line:column` places, or
// the expected outcome.
function answerAt(oracle, at) {
  const place = location => `${location.file}:${location.start.line}:${location.start.column}`;
  const token = oracle.tokens.find(token => place(token.location) === at);
  assert.ok(token, `no token at ${at}`);
  if (token.outcome !== 'groups') return token.outcome;
  return token.groups.map(index => ({
    declarations: oracle.groups[index].declarations.map(declaration => place(declaration.location)),
    occurrences: oracle.groups[index].occurrences.map(occurrence => place(occurrence.location)),
  }));
}

test('real oracle: exception fixtures record the compiler answers beside a different tool answer', { skip: oracleUnavailable }, async t => {
  const session = new SemanticSession(await loadSemanticApi());
  t.after(() => session.close());
  const cases = fs.readdirSync(exceptions, { withFileTypes: true }).filter(entry => entry.isDirectory()).map(entry => entry.name).sort();
  assert.ok(cases.length > 0);
  for (const name of cases) {
    const fixture = path.join(exceptions, name);
    const expected = JSON.parse(fs.readFileSync(path.join(fixture, 'expected.json'), 'utf8'));
    const oracle = normalizeOracle(session.collect(path.join(fixture, 'tsconfig.json')));
    assert.deepEqual(oracle.anomalies, [], name);
    for (const query of expected.queries) assert.deepEqual(answerAt(oracle, query.at), query.typescript, `${name}: ${query.at}`);
    // `tests/reference_exceptions.rs` checks the tool's side.
    assert.ok(expected.queries.some(query => !isDeepStrictEqual(query.typescript, query.tool)), `${name}: no recorded difference`);
  }
});

test('real compiler: replacement libraries are looked up from the configuration path as spelled', { skip: oracleUnavailable }, t => {
  // The compiler side of a documented difference: TypeScript finds an
  // `@typescript/lib-*` package beside a symbolic link to the configuration's
  // directory, where this tool looks from the real directory
  // (`tests/reference_libraries.rs` checks the tool side). The semantic
  // oracle opens the configuration's real path, so the native compiler
  // shows this.
  const directory = temporary(t);
  const write = (file, text) => {
    fs.mkdirSync(path.dirname(path.join(directory, file)), { recursive: true });
    fs.writeFileSync(path.join(directory, file), text);
  };
  write('real/tsconfig.json', '{ "compilerOptions": { "lib": ["es5", "es2015.promise"], "libReplacement": true }, "files": ["a.ts"] }');
  write('real/a.ts', 'export const n: number = replacedPromise;\n');
  write('spelled/node_modules/@typescript/lib-es2015/package.json', '{ "name": "@typescript/lib-es2015" }');
  write('spelled/node_modules/@typescript/lib-es2015/promise.d.ts', 'declare var replacedPromise: number;\n');
  fs.symlinkSync('../real', path.join(directory, 'spelled', 'link'));
  const tsc = (config, ...options) => spawnSync(process.env.TYPE_SCRIPT_NATIVE, ['-p', path.join(directory, config), ...options], { encoding: 'utf8' });
  const spelled = tsc('spelled/link/tsconfig.json', '--listFilesOnly');
  assert.equal(spelled.status, 0, spelled.stdout + spelled.stderr);
  assert.ok(spelled.stdout.includes('spelled/node_modules/@typescript/lib-es2015/promise.d.ts'), spelled.stdout);
  assert.equal(tsc('spelled/link/tsconfig.json', '--noEmit').status, 0);
  // From the real directory the name is unresolved (TS2304), as here.
  const real = tsc('real/tsconfig.json', '--noEmit', '--pretty', 'false');
  assert.match(real.stdout, /TS2304: Cannot find name 'replacedPromise'/);
});

test('real oracle: preserved symlinks keep distinct program files apart', { skip: oracleUnavailable }, async t => {
  const semantic = await loadSemanticApi();
  const session = new SemanticSession(semantic);
  t.after(() => session.close());
  const directory = temporary(t);
  fs.writeFileSync(path.join(directory, 'tsconfig.json'),
    '{ "compilerOptions": { "noLib": true, "preserveSymlinks": true }, "files": ["app.ts", "alias.ts"] }');
  fs.writeFileSync(path.join(directory, 'app.ts'), 'export const x = 1; x;\n');
  fs.symlinkSync('app.ts', path.join(directory, 'alias.ts'));
  const oracle = normalizeOracle(session.collect(path.join(directory, 'tsconfig.json')));
  assert.deepEqual(oracle.tokens.map(token => token.location.file).sort(), ['alias.ts', 'alias.ts', 'app.ts', 'app.ts']);
  assert.deepEqual(oracle.groups.map(group => group.occurrences.map(occurrence => occurrence.location.file)),
    [['alias.ts', 'alias.ts'], ['app.ts', 'app.ts']]);
});

test('real oracle: compare mode passes on exact answers and reports differences otherwise', { skip: oracleUnavailable }, async t => {
  const directory = temporary(t);
  const project = path.join(directory, 'project');
  fs.mkdirSync(project);
  fs.writeFileSync(path.join(project, 'tsconfig.json'), '{ "compilerOptions": { "noLib": true } }');
  fs.writeFileSync(path.join(project, 'a.ts'), 'const value = 1;\nvalue;\nlbl: for (;;) break lbl;\nmissing;\nexport {};\n');
  const semantic = await loadSemanticApi();
  const session = new SemanticSession(semantic);
  let oracle;
  try { oracle = normalizeOracle(session.collect(path.join(project, 'tsconfig.json'))); } finally { session.close(); }
  const kinds = { unsupported: 'UnsupportedTarget', unresolved: 'UnresolvedTarget' };
  const exact = oracle.tokens.map(token => token.outcome === 'groups'
    ? { result: { project: oracle.project, groups: resultGroups(oracle, token.groups) } }
    : { error: { kind: kinds[token.outcome], message: token.outcome } });
  // Name queries follow the position queries: here only `value` is a candidate.
  assert.deepEqual(nameQueries(oracle).map(query => query.name), ['value']);
  exact.push({ result: { project: oracle.project, groups: resultGroups(oracle, nameQueries(oracle)[0].candidates[0].groups) } });
  fs.writeFileSync(path.join(directory, 'answers.json'), JSON.stringify(exact));
  const batch = executable(directory, 'batch', `
    const fs = require('node:fs');
    const answers = JSON.parse(fs.readFileSync(${JSON.stringify(path.join(directory, 'answers.json'))}, 'utf8'));
    const lines = fs.readFileSync(0, 'utf8').split('\\n').filter(Boolean);
    lines.forEach((line, index) => console.log(JSON.stringify(process.env.EMPTY ? { result: { groups: [] } } : answers[index])));`);
  const run = environment => spawnSync(process.execPath, [conformance, 'compare', project],
    { cwd: directory, env: { ...process.env, REFERENCE_BATCH: batch, ...environment }, encoding: 'utf8', timeout: 60_000 });
  const passed = run({});
  assert.equal(passed.status, 0, passed.stdout + passed.stderr);
  const report = JSON.parse(fs.readFileSync(path.join(directory, '.cache/semantic-conformance.json'), 'utf8'));
  assert.deepEqual([report.queried, report.passed, report.failed], [oracle.tokens.length, oracle.tokens.length, 0]);
  assert.deepEqual(report.names, { queried: 1, passed: 1, failed: 0, byKind: {} });
  // `value` twice, the label twice, and `missing`.
  assert.deepEqual(report.outcomes, { groups: 2, unresolved: 1, unsupported: 2 });
  assert.deepEqual(report.normalizations, {});
  const failed = run({ EMPTY: '1' });
  assert.equal(failed.status, 1, failed.stdout + failed.stderr);
  const failures = JSON.parse(fs.readFileSync(path.join(directory, '.cache/semantic-conformance.json'), 'utf8'));
  assert.deepEqual(failures.byKind, { 'missing-group': 2, classification: 3 });
  assert.deepEqual(failures.names, { queried: 1, passed: 0, failed: 1, byKind: { 'missing-group': 1 } });
  assert.match(failed.stdout, /^FAIL .*project: name `value` \(1 candidates\): missing-group$/m);
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { Syntax, compareOutline, compilerKind, outlineExpectation, outlinePath } from './navigation-conformance.mjs';
import { expectationPath, sourceText } from './navigation-oracle.mjs';
import { loadCompilerApi } from './reference-oracle.mjs';

// Real-syntax tests are skipped, with this reason, unless the syntax API is configured.
const apiUnavailable = !process.env.TYPE_SCRIPT_API && 'TYPE_SCRIPT_API is not set';
const fixtures = fileURLToPath(new URL('../fixtures/navigation/', import.meta.url));

const at = (line, column) => ({ line, column });
const range = (line, column) => ({ start: at(line, column), end: at(line, column + 1) });

test('conformance maps catalog kinds to the compiler\'s symbol kinds', () => {
  assert.equal(compilerKind({ kind: 'type', label: 'T' }), 5);
  assert.equal(compilerKind({ kind: 'accessor', label: 'size' }), 7);
  assert.equal(compilerKind({ kind: 'signature', label: '()' }), 6);
  assert.equal(compilerKind({ kind: 'signature', label: 'new()' }), 9);
  assert.equal(compilerKind({ kind: 'signature', label: '[]' }), 7);
  assert.equal(compilerKind({ kind: 'module', label: '"m"' }), 3);
});

test('allowances explain only the entries their own syntax classifies', () => {
  const entry = (label, kind, line, column, children = []) => ({ label, kind, anchor: at(line, column), range: range(line, column), children });
  const symbol = (name, kind, line, column, children = []) =>
    ({ name, kind, range: range(line, column), selection: range(line, column), children });
  // Classifications by label; every other fact holds.
  const syntax = ({ absent = {}, transparent = [], mergeable = true } = {}) => ({
    absence: node => absent[node.label] ?? null,
    transparent: node => transparent.includes(node.label),
    mergeable: () => mergeable,
    kindAgrees: () => true,
    statement: () => false,
  });

  // A shorthand the compiler lists, and a property it merged into a function.
  let result = compareOutline({
    entries: [entry('f', 'function', 1, 1, [entry('run', 'property', 2, 1, [entry('inner', 'function', 3, 1)]),
      entry('run', 'function', 5, 1)])],
    symbols: [symbol('f', 12, 1, 1, [symbol('short', 7, 2, 5), symbol('run', 12, 5, 1, [symbol('inner', 12, 3, 1)])])],
    syntax: syntax({ absent: { short: 'shorthand' } }),
  });
  assert.deepEqual(result.differences, []);
  assert.deepEqual(result.allowances, { shorthand: 1, merged: 2 });
  // Merging requires property-like syntax at the catalog entry.
  result = compareOutline({
    entries: [entry('f', 'function', 1, 1, [entry('run', 'property', 2, 1), entry('run', 'function', 5, 1)])],
    symbols: [symbol('f', 12, 1, 1, [symbol('run', 12, 5, 1)])],
    syntax: syntax({ mergeable: false }),
  });
  assert.deepEqual(result.differences.map(difference => difference.kind), ['extra']);

  // An omitted wrapper's descendants are not excused with it.
  result = compareOutline({
    entries: [entry('f', 'function', 1, 1)],
    symbols: [symbol('f', 12, 1, 1, [symbol('x', 13, 2, 1, [symbol('x', 12, 2, 5, [symbol('keep', 12, 3, 1)])])])],
    syntax: syntax({ absent: { x: 'expando' } }),
  });
  assert.deepEqual(result.differences.map(difference => [difference.kind, difference.label]), [['missing', 'keep']]);
  assert.deepEqual(result.allowances, { expando: 2 });

  // Contents of a spread are catalog-only, and each needs its own reason.
  result = compareOutline({
    entries: [entry('o', 'variable', 1, 1, [entry('run', 'method', 1, 9, [entry('inner', 'function', 1, 20)])])],
    symbols: [symbol('o', 13, 1, 1, [symbol('{ run() {} }', 7, 1, 8)])],
    syntax: syntax({ absent: { '{ run() {} }': 'shorthand' }, transparent: ['run'] }),
  });
  assert.deepEqual(result.differences.map(difference => [difference.kind, difference.label]), [['extra', 'inner']]);

  // Kind disagreements, missing entries, and extra entries are unexplained.
  result = compareOutline({
    entries: [entry('a', 'variable', 1, 1), entry('extra', 'function', 4, 1)],
    symbols: [symbol('a', 12, 1, 1), symbol('missing', 13, 2, 1)],
    syntax: syntax(),
  });
  assert.deepEqual(result.differences.map(difference => difference.kind).sort(), ['extra', 'kind', 'missing']);

  // Only the compiler truncates labels.
  const long = 'x'.repeat(160);
  result = compareOutline({
    entries: [entry(long, 'variable', 1, 1)], symbols: [symbol(`${long.slice(0, 150)}...`, 13, 1, 1)], syntax: syntax(),
  });
  assert.deepEqual(result.allowances, { truncated: 1 });
  assert.deepEqual(result.differences, []);
});

test('pinned outlines are written one entry per line', () => {
  const entries = [{ label: 'a', kind: 'function', children: [{ label: 'b', kind: 'variable', children: [] }] }];
  const text = outlineExpectation(entries);
  assert.equal(text, '{\n  "entries": [\n    {"label":"a","kind":"function", "children": [\n'
    + '      {"label":"b","kind":"variable", "children": []}\n    ]}\n  ]\n}\n');
  assert.deepEqual(JSON.parse(text).entries, entries);
  assert.equal(outlineExpectation([]), '{\n  "entries": []\n}\n');
});

// Real syntax: each fixture's pinned catalog entries (`<file>.outline.json`,
// kept current by tests/navigation_cli.rs) against its pinned compiler
// symbols (`<file>.symbols.json`, kept current by navigation-oracle.test.mjs).
let ts;
function pinned(name) {
  ts ??= loadCompilerApi();
  const file = path.join(fixtures, name);
  const position = text => { const [line, column] = text.split(':').map(Number); return at(line, column); };
  const span = text => { const [start, end] = text.split('-'); return { start: position(start), end: position(end) }; };
  const symbols = list => list.map(symbol => ({ name: symbol.name, kind: symbol.kind, range: span(symbol.range),
    selection: span(symbol.selection), children: symbols(symbol.children) }));
  return {
    entries: JSON.parse(fs.readFileSync(outlinePath(file), 'utf8')).entries,
    symbols: symbols(JSON.parse(fs.readFileSync(expectationPath(file), 'utf8')).symbols),
    syntax: new Syntax(ts, file, sourceText(file)),
  };
}

// The entry at a label path, with the list holding it.
function locate(entries, labels) {
  let list = entries, entry;
  for (const label of labels) {
    const index = list.findIndex(candidate => candidate.label === label);
    assert.ok(index >= 0, `no ${labels.join(' > ')}`);
    entry = list[index];
    if (label !== labels.at(-1)) list = entry.children;
  }
  return { list, entry };
}

function mutated(name, mutate) {
  const input = pinned(name);
  mutate(input.entries);
  return compareOutline(input);
}

const summary = result => result.differences.map(difference => `${difference.kind} ${difference.label}`).sort();

test('real syntax: the pinned outlines differ only as documented', { skip: apiUnavailable }, () => {
  let result = compareOutline(pinned('expandos.ts'));
  assert.deepEqual(result.differences, []);
  assert.deepEqual(result.allowances, { expando: 4, merged: 3 });
  assert.equal(result.checked.ranges, result.matched);
  // The `...defaults` spread; the returned `startEligible` shorthand is
  // merged into its function by the compiler, so it is no entry there.
  result = compareOutline(pinned('factory.ts'));
  assert.deepEqual(result.differences, []);
  assert.deepEqual(result.allowances, { shorthand: 1 });
  assert.equal(result.checked.ranges, result.matched);
  // Later `declare module "pkg"` and `declare global` blocks, which the
  // compiler merges into the first, and the copies of two properties'
  // callbacks that it lists under both same-named variables (Command
  // Center's `@tiptap/core` declarations and `checkpoint-handoff.ts`).
  result = compareOutline(pinned('repeated-merges.ts'));
  assert.deepEqual(result.differences, []);
  assert.deepEqual(result.allowances, { merged: 12 });
  assert.equal(result.checked.ranges, result.matched);
});

test('real syntax: merges explain only what the compiler merged', { skip: apiUnavailable }, () => {
  // A later module block of another name merges into nothing.
  let result = mutated('repeated-merges.ts', entries => { entries.filter(entry => entry.label === '"pkg"')[1].label = '"pkg2"'; });
  assert.deepEqual(summary(result), ['containment Commands', 'extra "pkg2"']);
  // Without a property of their name, nothing merged into the variables, so
  // the callbacks listed under them are unexplained.
  result = mutated('repeated-merges.ts', entries => {
    for (const child of locate(entries, ['publish']).entry.children) if (child.kind === 'property') child.label = 'other';
  });
  assert.ok(summary(result).includes('extra other'), summary(result).join(', '));
  assert.ok(summary(result).includes('missing records.map() callback'), summary(result).join(', '));
});

test('real syntax: dropping an expando\'s assigned function or its contents fails', { skip: apiUnavailable }, () => {
  let result = mutated('expandos.ts', entries => {
    const { list, entry } = locate(entries, ['method']);
    list.splice(list.indexOf(entry), 1);
  });
  assert.deepEqual(summary(result), ['missing keepMe', 'missing method']);
  result = mutated('expandos.ts', entries => { locate(entries, ['method']).entry.children = []; });
  assert.deepEqual(summary(result), ['missing keepMe']);
  result = mutated('expandos.ts', entries => { locate(entries, ['outer', 'attached']).entry.children = []; });
  assert.deepEqual(summary(result), ['missing keepInner']);
});

test('real syntax: reparenting a declaration under an expando fails', { skip: apiUnavailable }, () => {
  // To file level, out of the assigned function.
  let result = mutated('expandos.ts', entries => {
    const method = locate(entries, ['method']).entry;
    entries.push(...method.children);
    method.children = [];
  });
  assert.deepEqual(summary(result), ['containment keepMe']);
  // Under the expando's target, where the compiler shows the wrapper.
  result = mutated('expandos.ts', entries => {
    const outer = locate(entries, ['outer']).entry;
    const [inner, attached] = outer.children;
    outer.children = [{ ...inner, children: [attached] }];
  });
  assert.deepEqual(summary(result), ['containment attached']);
});

test('real syntax: listing the returned shorthand fails', { skip: apiUnavailable }, () => {
  // `return { startEligible, … }` beside `function startEligible`: the
  // compiler merges its shorthand entry away, but a shorthand is never a
  // catalog entry, so `merged` cannot explain it.
  const result = mutated('factory.ts', entries => {
    const host = locate(entries, ['createRunHost']).entry;
    host.children.splice(3, 0, {
      label: 'startEligible', key: 'startEligible', selector: 'createRunHost.startEligible', kind: 'property',
      anchor: at(13, 12), range: { start: at(13, 12), end: at(13, 25) }, signature: 'startEligible', children: [],
    });
  });
  assert.deepEqual(summary(result), ['extra startEligible', 'syntax startEligible']);
});

test('real syntax: a wrong range or fine kind fails', { skip: apiUnavailable }, () => {
  // Another function's range: the right syntax, but not this site's.
  let result = mutated('factory.ts', entries => {
    const other = locate(entries, ['createRunHost', 'startEligible']).entry;
    locate(entries, ['createRunHost', 'helper', 'step']).entry.range = other.range;
  });
  assert.deepEqual(summary(result), ['range step']);
  // A range that is no syntax at all.
  result = mutated('factory.ts', entries => {
    const step = locate(entries, ['createRunHost', 'helper', 'step']).entry;
    step.range = { ...step.range, end: { ...step.range.end, column: step.range.end.column - 1 } };
  });
  assert.deepEqual(summary(result), ['range step', 'syntax step']);
  // A binding's statement must contain the compiler's declaration.
  result = mutated('factory.ts', entries => {
    const helper = locate(entries, ['createRunHost', 'helper']).entry;
    helper.range = { start: helper.anchor, end: at(9, 15) };
  });
  assert.deepEqual(summary(result), ['range helper', 'syntax helper']);
  // Pairs the compiler's symbol kinds cannot tell apart.
  for (const [labels, kind] of [[['Widget', 'size'], 'property'], [['settings'], 'import'], [['Widget'], 'type'],
    [['Widget', 'label'], 'accessor']]) {
    result = mutated('factory.ts', entries => { locate(entries, labels).entry.kind = kind; });
    assert.deepEqual(summary(result), [`syntax ${labels.at(-1)}`], `${labels.join('.')} as ${kind}`);
  }
});

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { COMPARISON_OPTIONS, describeComparison, firstDifference, normalizer } from './ast-comparison.mjs';

test('equal JSON trees compare equally regardless of object property order', () => {
  const actual = { kind: 308, statements: [{ text: 'x', kind: 80 }], empty: [], optional: null };
  const expected = { optional: null, empty: [], statements: [{ kind: 80, text: 'x' }], kind: 308 };
  assert.equal(firstDifference(actual, expected), undefined);
});

test('arrays cannot compare equally to objects with the same enumerable keys', () => {
  for (const [actual, expected] of [[[], {}], [[1, 2], { 0: 1, 1: 2 }]]) {
    for (const [left, right] of [[actual, expected], [expected, actual]]) {
      assert.deepEqual(firstDifference({ arguments: left }, { arguments: right }), {
        path: '$.arguments', actual: left, expected: right,
      });
    }
  }
});

test('array lengths are compared even when their elements are absent or undefined', () => {
  for (const [actual, expected] of [[[undefined], []], [new Array(2), new Array(1)]]) {
    assert.deepEqual(firstDifference({ arguments: actual }, { arguments: expected }), {
      path: '$.arguments.length', actual: actual.length, expected: expected.length,
    });
  }
});

test('missing own properties differ from present undefined values and inherited values', () => {
  assert.deepEqual(firstDifference({ name: undefined }, {}), {
    path: '$.name', actual: undefined, expected: undefined,
    actualMissing: false, expectedMissing: true,
  });
  assert.deepEqual(firstDifference(Object.create({ name: 'x' }), { name: 'x' }), {
    path: '$.name', actual: undefined, expected: 'x',
    actualMissing: true, expectedMissing: false,
  });
  assert.deepEqual(firstDifference(new Array(1), [undefined]), {
    path: '$.0', actual: undefined, expected: undefined,
    actualMissing: true, expectedMissing: false,
  });
});

test('the first mismatch follows deterministic property and numeric element order', () => {
  const actual = { z: 0, a: [0, 1, 2] };
  const expected = { a: [0, 9, 8], z: 1 };
  assert.deepEqual(firstDifference(actual, expected), { path: '$.a.1', actual: 1, expected: 9 });
  assert.deepEqual(firstDifference({ z: undefined, a: 1 }, { a: 2 }), {
    path: '$.a', actual: 1, expected: 2,
  });
  const numbers = Array.from({ length: 12 }, (_, index) => index);
  const changed = [...numbers];
  changed[2] = 20;
  changed[10] = 100;
  assert.deepEqual(firstDifference(numbers, changed), { path: '$.2', actual: 2, expected: 20 });
});

test('normalization keeps syntax array shape and selected metadata while omitting compiler caches', () => {
  const statements = [{ kind: 11, flags: 2097152, pos: 0, end: 2, text: 'x', rawText: 'raw' }];
  statements.pos = 0;
  statements.end = 2;
  const expected = { kind: 308, text: 'original source', fileName: 'source.ts', statements };
  const actual = { kind: 308, text: 'another source', statements: [
    { kind: 11, flags: 0, pos: 0, end: 2, text: 'x', rawText: 'raw' },
  ] };
  const normalize = normalizer(new Set(['--positions', '--flags', '--scanner-hints']));
  assert.equal(firstDifference(normalize(actual, true), normalize(expected, true)), undefined);
  assert.deepEqual(normalizer()(expected, true), { kind: 308, statements: [{ kind: 11, text: 'x' }] });
  assert.equal(firstDifference(normalize({ statements: [] }), normalize({ statements: {} })).path,
    '$.statements');
});

test('each comparison option restores only its own fields and is reported as active', () => {
  const node = { kind: 11, pos: 1, end: 2, flags: 2097152 | 16, rawText: 'r', templateFlags: 1,
    hasExtendedUnicodeEscape: true, numericLiteralFlags: 8, text: 'x' };
  const restored = {
    '--positions': { pos: 1, end: 2 },
    '--flags': { flags: 16 },
    '--scanner-hints': { rawText: 'r', templateFlags: 1, hasExtendedUnicodeEscape: true, numericLiteralFlags: 8 },
  };
  assert.deepEqual(COMPARISON_OPTIONS, Object.keys(restored));
  const structure = normalizer();
  assert.deepEqual(structure(node), { kind: 11, text: 'x' });
  assert.deepEqual(structure.compared, []);
  assert.equal(describeComparison(structure.compared), 'structure only');
  for (const option of COMPARISON_OPTIONS) {
    // Flags that are not comparison options, such as a runner's own, are ignored.
    const normalize = normalizer(new Set([option, '--check-only']));
    assert.deepEqual(normalize(node), { kind: 11, text: 'x', ...restored[option] }, option);
    assert.deepEqual(normalize.compared, [option.slice(2)]);
    assert.equal(describeComparison(normalize.compared), option.slice(2));
  }
  const all = normalizer(new Set([...COMPARISON_OPTIONS].reverse()));
  assert.deepEqual(all.compared, ['positions', 'flags', 'scanner-hints']);
  assert.equal(describeComparison(all.compared), 'positions, flags, scanner-hints');
  assert.equal(describeComparison(all.compared, { trees: false }), 'acceptance only');
});

test('normalization takes parsed runner flags, not raw arguments', () => {
  for (const raw of [['--positions'], process.argv, '--positions'])
    assert.throws(() => normalizer(raw), /flag set parsed by runnerArguments/);
});

test('normalization and comparison handle deep trees without a recursive traversal stack', () => {
  const depth = 20_000;
  const tree = value => {
    let node = { value };
    for (let index = 0; index < depth; index++) node = { child: [node] };
    return node;
  };
  const normalize = normalizer();
  const actual = normalize(tree(1));
  assert.equal(firstDifference(actual, normalize(tree(1))), undefined);
  const difference = firstDifference(actual, normalize(tree(2)));
  assert.equal(difference.path, '$' + '.child.0'.repeat(depth) + '.value');
  assert.equal(difference.actual, 1);
  assert.equal(difference.expected, 2);
});

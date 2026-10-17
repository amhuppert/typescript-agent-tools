// Pinned site-sensitive reads establish inputs for native restricted flow admission.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { typeOracle, compilerHookOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/native-read-admission/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'reads.ts'), 'utf8');
function at(marker) {
  const comment = `/*${marker}*/ `;
  const offset = source.indexOf(comment);
  assert.notEqual(offset, -1, marker);
  assert.equal(source.indexOf(comment, offset + 1), -1, marker);
  return { file: 'reads.ts', ...position(lineStarts(source), offset + comment.length) };
}
async function observe(config, marker, summary) {
  const answer = await typeOracle(path.join(directory, config), { at: at(marker), properties: false });
  assert.equal(answer.compiler, '7.0.2');
  assert.equal(answer.history, 'baseline');
  assert.equal(answer.kind, 'expression');
  assert.equal(answer.summary, summary, `${config}: ${marker}`);
  assert.equal(answer.errorType, false);
  assert.deepEqual(answer.prelude, []);
  return answer;
}

test('ordinary initialized nonunion, ambient union and unrelated assignments preserve exact read types', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [marker, summary] of [
      ['fixed-read', '1'], ['count-read', 'number'], ['declared-read', 'string | number'],
      ['after-unrelated-assignment', 'number'], ['after-own-assignment', 'number'],
    ]) await observe(config, marker, summary);
  }
});

test('union initializer reduction and automatic reads are real distinct flow operations', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await observe(config, 'refined-read', 'number');
    await observe(config, 'flag-read', 'false');
    await observe(config, 'automatic-read', config === 'tsconfig.json' ? 'undefined' : 'any');
  }
});

test('actual conditions, unreachable nodes and loops retain the compiler site-sensitive outcomes', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [marker, summary] of [
      ['inside-unrelated-condition', 'number'], ['inside-own-condition', 'number'],
      ['inside-constant-condition', 'number'], ['inside-unreachable-condition', 'string | number'],
      ['inside-unreachable-loop', 'number'], ['after-unreachable-loop', 'number'],
    ]) await observe(config, marker, summary);
  }
});

test('function execution boundaries distinguish local graph starts from captured variables', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await observe(config, 'local-start-read', 'number');
    await observe(config, 'outer-function-read', 'number');
  }
});

test('same-spelling shadowed assignments and conditions use actual symbol identity', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await observe(config, 'shadowed-inner-read', 'string');
    await observe(config, 'after-shadowed-assignment', 'number');
    await observe(config, 'after-shadowed-condition', 'number');
  }
});

test('a never antecedent returns before consuming the matching condition narrowing', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await observe(config, 'never-condition-read', 'never');
  }
});

test('matching an unrelated unknown symbol does not consume its error type', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await observe(config, 'unknown-condition-read', 'number');
  }
});

test('assignment positions and uninitialized diagnostics have distinct native read questions', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await observe(config, 'definite-union-write', 'string | number');
    await observe(config, 'logical-union-write', 'string | number');
    await observe(config, 'numeric-compound-write', 'number');
    const answer = await typeOracle(path.join(directory, config), { at: at('constant-write'), properties: false });
    assert.equal(answer.summary, 'any');
    assert.equal(answer.errorType, true);
    assert.equal(answer.kind, 'expression');
    await observe(config, 'uninitialized-number-read', 'number');
  }
});

test('every native assignment target kind and base literal conversion retains its own question', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [marker, summary] of [
      ['compound-plus', 'number'], ['compound-minus', 'number'],
      ['compound-multiply', 'number'], ['compound-power', 'number'],
      ['compound-divide', 'number'], ['compound-remainder', 'number'],
      ['compound-and', 'number'], ['compound-or', 'number'],
      ['compound-xor', 'number'], ['compound-shift-left', 'number'],
      ['compound-shift-right', 'number'], ['compound-shift-unsigned', 'number'],
      ['compound-postfix', 'number'], ['compound-prefix', 'number'],
      ['literal-definite', '1'], ['literal-compound', 'number'],
      ['literal-compound-like', 'number'], ['literal-bitwise-rhs', '1'],
      ['literal-union-definite', '1 | 2'], ['literal-union-compound-like', 'number'],
      ['logical-and-write', 'string | number'], ['logical-nullish-write', 'string | number'],
      ['for-in-write', 'string | number'], ['for-of-write', 'string | number'],
    ]) await observe(config, marker, summary);
  }
});

test('matching flow assignments consume compound antecedents and simple arithmetic base types', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await observe(config, 'after-literal-compound', 'number');
    await observe(config, 'after-literal-compound-like', 'number');
    await observe(config, 'after-ambient-union-compound', 'string | number');
  }
});

test('direct and parenthesized non-null operands preserve their own read setup', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    await observe(config, 'nonnull-initialization-read', 'number');
    await observe(config, 'parenthesized-nonnull-read', 'number');
  }
});

test('alias reads use their own raw symbols and read containers', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const [marker, summary] of [['first-alias-read', 'number'], ['second-alias-read', 'number'], ['union-alias-read', 'string | number'], ['function-alias-read', '() => void'], ['captured-alias-read', 'number'], ['captured-alias-arrow-read', 'number'], ['local-arrow-nonnull-read', 'number'], ['other-alias-condition-read', 'number'], ['same-alias-condition-read', 'number']]) {
      await observe(config, marker, summary);
    }
  }
});


test('all native iterative statement forms preserve ordinary identity reads', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const marker of ['inside-while-identity', 'after-while-identity', 'inside-do-identity', 'after-do-identity', 'inside-for-unreachable', 'after-for-identity', 'inside-for-in-identity', 'after-for-in-identity', 'inside-for-of-identity', 'after-for-of-identity', 'inside-cyclic-identity', 'after-infinite-loop']) {
      await observe(config, marker, 'number');
    }
  }
});


test('labels retain exact nested targets and execution-container boundaries', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const marker of ['before-labelled-break', 'after-labelled-break-unreachable', 'after-labelled-break', 'before-labelled-continue', 'after-labelled-continue-unreachable', 'after-labelled-infinite-loop', 'inside-labelled-block', 'after-block-break-unreachable', 'after-labelled-block', 'before-escaped-label-break', 'after-escaped-label', 'invalid-label-target-read']) {
      await observe(config, marker, 'number');
    }
  }
});


test('native effect-signature selection distinguishes ordinary calls from assertions and never returns', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const marker of ['after-ordinary-call', 'after-generic-ordinary-call', 'after-annotated-call', 'after-overload-ordinary-call', 'after-untyped-call', 'after-unresolved-call', 'after-comma-ordinary-calls', 'after-nonasserting-predicate-call', 'after-asserting-call', 'after-never-call']) {
      await observe(config, marker, 'number');
    }
  }
});

test('assertion reachability uses the complete native syntactic false question', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const marker of ['after-false-assertion-assignment', 'after-parenthesized-false-assertion', 'after-and-left-false-assertion', 'after-and-right-false-assertion', 'after-or-false-assertion']) {
      await observe(config, marker, 'string | number');
    }
    for (const marker of ['after-true-assertion-assignment', 'after-true-or-assertion']) {
      await observe(config, marker, 'string');
    }
  }
});

test('logical and nullish binder branches preserve identity and reset pure top-level flow', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const marker of ['after-pure-logical-and', 'after-pure-logical-or', 'after-pure-logical-nullish', 'after-negated-pure-logical', 'after-logical-and-effects', 'after-logical-or-effects', 'after-logical-nullish-effects', 'inside-nested-logical-condition', 'inside-negated-logical-condition']) {
      await observe(config, marker, 'number');
    }
  }
});

test('assertion narrowing consumes its antecedent and preserves native truthiness identities', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const marker of ['inside-matching-number-truthy', 'inside-matching-number-falsy', 'after-matching-number-assertion', 'after-number-or-false-assertion', 'after-false-or-number-assertion', 'after-number-and-true-assertion', 'after-zero-assertion', 'after-null-assertion', 'after-negated-true-assertion']) {
      await observe(config, marker, 'number');
    }
    await observe(config, 'inside-matching-literal-true', 'true');
    await observe(config, 'inside-matching-literal-false', config === 'tsconfig.json' ? 'never' : 'true');
    await observe(config, 'false-assertion-consumed-union-antecedent', 'string | number');
    await observe(config, 'after-direct-false-assertion', 'number');
    await observe(config, 'after-direct-and-false-assertion', 'number');
    await observe(config, 'after-direct-or-false-assertion', 'never');
  }
});

test('unrelated unknown symbols retain acquired resolution failures under healthy roots', async () => {
  for (const config of ['tsconfig.json', 'loose.json']) {
    for (const marker of ['unknown-condition-read', 'after-unresolved-call', 'after-comma-ordinary-calls']) {
      const answer = await compilerHookOracle(path.join(directory, config), { at: at(marker) });
      assert.equal(answer.summary, 'number');
      assert.equal(answer.errorType, false);
      assert.ok(answer.beforeSummary.diagnosticCodes.includes(2304), `${config}/${marker}: ${JSON.stringify(answer.beforeSummary)}`);
    }
  }
});

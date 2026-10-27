import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';

const globals = `interface Object {}\ninterface Function {}\ninterface Array<T> { length: number; [n: number]: T }\ninterface ReadonlyArray<T> { readonly length: number; readonly [n: number]: T }\ninterface String { readonly length: number }\ninterface Number {}\ninterface Boolean {}\ninterface RegExp {}\ninterface IArguments {}\ninterface CallableFunction extends Function {}\ninterface NewableFunction extends Function {}\n`;

async function observe(t, name, body, noCheck = false, acquisitions = true) {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-conditional-cache-')));
  t.after(() => fs.rmSync(directory, {recursive: true, force: true}));
  const source = globals + body;
  fs.writeFileSync(path.join(directory, 'cases.ts'), source);
  const config = path.join(directory, 'tsconfig.json');
  fs.writeFileSync(config, JSON.stringify({compilerOptions: {strict: true, noLib: true, noCheck}, files: ['cases.ts']}));
  const marker = '/*#query*/';
  const at = {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(marker) + marker.length)};
  const native = await compilerHookOracle(config, {at, history: 'cold', inspectConditionalCaches: true,
    inspectDiagnosticCache: acquisitions, inspectExpressionAcquisitions: acquisitions});
  const api = await typeOracle(config, {at, history: 'cold', properties: false});
  assert.equal(native.checkerCount, 1);
  assert.deepEqual([native.summary, native.errorType], [api.summary, api.errorType], name);
  const observed = {name, noCheck, source, summary: native.summary, errorType: native.errorType,
    counters: native.beforeSummary.counters, failures: native.beforeSummary.demandFailures,
    codes: native.beforeSummary.diagnosticCodes,
    tail: native.beforeSummary.observations.conditionalTailLocation,
    restrictive: Object.entries(native.beforeSummary.observations).filter(([key]) => key.startsWith('restrictive:'))
      .map(([key, value]) => ({key, ...value})),
    errors: (native.beforeSummary.displayAcquisitions ?? []).filter(event => event.event === 'exit' && event.errorType)};
  fs.writeFileSync(scratchPath(`stage60-conditional-${name}-${noCheck}.json`), JSON.stringify(observed, null, 2) + '\n');
  return observed;
}

test('native conditional restrictive cache candidate final queries', async t => {
  const cases = [
    ['outerMissing', 'type Outer<T> = { value: MissingOuter } extends {} ? 51 : 0;\ndeclare const /*#query*/query: Outer<number>;\n'],
    ['outerQuery', 'type Outer<T> = { value: typeof missingOuter } extends {} ? 52 : 0;\ndeclare const /*#query*/query: Outer<number>;\n'],
    ['simplified', 'type Filter<T> = T extends { value: MissingFilter } ? T : never;\nfunction f<T>(value: Filter<T>) { const /*#query*/query = value.value; }\n'],
    ['simplifiedQuery', 'type Filter<T> = T extends { value: typeof missingFilter } ? T : never;\nfunction f<T>(value: Filter<T>) { const /*#query*/query = value.value; }\n'],
    ['repeat', 'type Shape<T> = { value: MissingRepeat };\ntype Both<T> = [Shape<T> extends {} ? 53 : 0, Shape<T> extends object ? 54 : 0];\ndeclare const /*#query*/query: Both<unknown>;\n'],
    ['inferConstraint', 'type Shape<T> = { value: T };\ntype Inferred<T> = T extends Shape<infer U> ? { value: MissingInference } extends U ? 55 : 0 : 0;\ndeclare const /*#query*/query: Inferred<Shape<{}>>;\n'],
  ];
  const rows = [];
  for (const [name, body] of cases) {
    rows.push(await observe(t, name, body));
    fs.writeFileSync(scratchPath('stage60-restrictive-candidates.json'), JSON.stringify(rows, null, 2) + '\n');
  }
  assert.equal(rows.length, 6);
});

test('native conditional tail exact boundary and later acquired operand', async t => {
  const build = "type Build<N extends number, T extends unknown[] = []> = T['length'] extends N ? T : Build<N, [...T, unknown]>;\n";
  const rows = [];
  for (const noCheck of [false, true]) {
    for (const count of [999, 1000, 1001]) {
      const row = await observe(t, `tail${count}`, build + `type /*#query*/Result = Build<${count}>['length'];\n`, noCheck, false);
      assert.equal(row.summary, count === 999 ? '999' : 'any');
      assert.equal(row.errorType, count !== 999);
      assert.equal(row.counters.conditionalTail, Math.min(count, 1000));
      assert.deepEqual(row.failures, count === 999 ? [] : ['conditional_tail_limit']);
      assert.deepEqual(row.codes, count === 999 ? [] : [2589]);
      // Direct getTypeFromTypeNode does not install c.currentNode. Native
      // errorType recovery is observable without inventing a source span.
      assert.equal(row.tail, undefined);
      rows.push(row);
      fs.writeFileSync(scratchPath('stage60-conditional-tail.json'), JSON.stringify(rows, null, 2) + '\n');
    }
    const row = await observe(t, 'tailContinuation', build + 'type /*#query*/Result = [Build<1000>, MissingAfterTail][number];\n', noCheck, true);
    assert.deepEqual(row.failures, ['conditional_tail_limit']);
    assert.equal(row.counters.conditionalTail, 1000);
    assert.deepEqual(row.codes, [2589, 2304]);
    assert.equal(row.tail, undefined);
    rows.push(row);
    fs.writeFileSync(scratchPath('stage60-conditional-tail.json'), JSON.stringify(rows, null, 2) + '\n');
    for (const [count, later] of [[999, false], [1000, false], [1001, false], [1000, true]]) {
      const target = later ? `[Build<${count}>, MissingAfterTail][number]` : `Build<${count}>`;
      const expression = await observe(t, later ? 'tailExpressionContinuation' : `tailExpression${count}`,
        build + `const /*#query*/query = (null as ${target}).length;\n`, noCheck, true);
      rows.push(expression);
      fs.writeFileSync(scratchPath('stage60-conditional-tail.json'), JSON.stringify(rows, null, 2) + '\n');
      assert.equal(expression.summary, count === 999 ? '999' : 'any');
      assert.equal(expression.errorType, count !== 999);
      assert.deepEqual(expression.failures, count === 999 ? [] : ['conditional_tail_limit']);
      assert.equal(expression.counters.conditionalTail, Math.min(count, 1000));
      assert.deepEqual(expression.codes, count === 999 ? [] : later ? [2589, 2304] : [2589]);
      if (count === 999) assert.equal(expression.tail, undefined);
      else assert.equal(expression.source.slice(expression.tail.pos, expression.tail.end).trim(), `Build<${count}>`);
    }
  }
});

test('native lightweight string transitions retain the same conditional tail boundary', async t => {
  const build = 'type Next<S extends string> = S extends `x${infer R}` ? R : never;\n'
    + 'type Build<S extends string> = S extends "" ? { length: 999 } : Build<Next<S>>;\n';
  const rows = [];
  for (const noCheck of [false, true]) {
    for (const [count, later] of [[999, false], [1000, false], [1001, false], [1000, true]]) {
      const reference = `Build<"${'x'.repeat(count)}">`;
      const target = later ? `[${reference}, MissingAfterTail][number]` : reference;
      const start = performance.now();
      const row = await observe(t, later ? 'lightweightContinuation' : `lightweight${count}`,
        build + `const /*#query*/query = (null as ${target}).length;\n`, noCheck, true);
      row.elapsedMilliseconds = performance.now() - start;
      rows.push(row);
      fs.writeFileSync(scratchPath('stage62-conditional-tail-lightweight.json'), JSON.stringify(rows, null, 2) + '\n');
      assert.equal(row.summary, count === 999 ? '999' : 'any');
      assert.equal(row.errorType, count !== 999);
      assert.deepEqual(row.failures, count === 999 ? [] : ['conditional_tail_limit']);
      assert.equal(row.counters.conditionalTail, Math.min(count, 1000));
      assert.deepEqual(row.codes, count === 999 ? [] : later ? [2589, 2304] : [2589]);
      if (count === 999) assert.equal(row.tail, undefined);
      else assert.equal(row.source.slice(row.tail.pos, row.tail.end).trim(), reference);
    }
  }
});

test('native restrictive same-result candidate installs then observes its own cache', async t => {
  const body = 'type Filter<T> = T extends { value: MissingFilter } ? T : never;\n'
    + 'function f<T>(value: Filter<T>) { const /*#query*/query = value.value; }\n'
    + 'const healthy = 45 as const;\n';
  const rows = [];
  for (const noCheck of [false, true]) {
    const row = await observe(t, 'restrictiveMarker', body, noCheck, true);
    rows.push(row);
    fs.writeFileSync(scratchPath('stage62-restrictive-marker.json'), JSON.stringify(rows, null, 2) + '\n');
    assert.equal(row.summary, 'MissingFilter');
    assert.equal(row.errorType, true);
    assert.deepEqual(row.codes, [2304]);
    assert.deepEqual(row.failures, []);
    const installed = row.restrictive.filter(event => event.same && !event.cacheHit
      && !event.cacheSelfBefore && event.cacheSelfAfter);
    assert.equal(installed.length, 2);
    for (const event of installed) {
      assert.ok(row.restrictive.some(hit => hit.input === event.input && hit.same
        && hit.cacheHit && hit.cacheSelfBefore && hit.cacheSelfAfter));
    }
  }
});

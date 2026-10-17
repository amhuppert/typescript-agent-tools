import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lineStarts, position } from './semantic-oracle.mjs';
import { typeOracle, memberOracle, compilerHookOracle } from './type-oracle.mjs';
import { crossProductSource, headRecursiveSource, instantiationCountSource,
  relationDepthSource, subtypeRemovalSource, relationMaybeSource, relationAllowanceSource } from '../fixtures/types/compiler-limits/generate.mjs';

const fixtures = fileURLToPath(new URL('../fixtures/types/', import.meta.url));
function at(fixture, marker, file = 'cases.ts') {
  const source = fs.readFileSync(path.join(fixtures, fixture, file), 'utf8');
  const token = `/*#${marker}*/`;
  const offset = source.indexOf(token);
  assert.notEqual(offset, -1, marker);
  return { file, ...position(lineStarts(source), offset + token.length) };
}
const config = fixture => path.join(fixtures, fixture, 'tsconfig.json');

test('synthetic typeof keywords demand their real containing TypeQuery node', async () => {
  for (const [site, summary] of [['type Narrowed = typeof', 'string'], ['type Assigned = typeof', 'number'],
    ['type Guarded = typeof', '{ value: string; }']]) {
    const request = { at: textAt('value-read', site, 'typeof', 'cases.ts'), properties: false };
    const hook = await compilerHookOracle(config('value-read'), request);
    assert.deepEqual([hook.kind, hook.summary, hook.errorType], ['type', summary, false]);
    let answer;
    await assert.doesNotReject(async () => { answer = await typeOracle(config('value-read'), request); },
      'a valid TypeQuery anchor must return its native type instead of asking an unused synthetic-token symbol');
    assert.deepEqual([answer.kind, answer.summary, answer.errorType], ['type', summary, false]);
  }
});

test('typeof operands ask the site-sensitive native value read while explicit symbol controls retain declared types', async () => {
  for (const [marker, summary, declared] of [['narrowed', 'string', 'string | number'],
    ['assigned', 'number', 'string | number'], ['guarded', '{ value: string; }', '{ value: string; } | undefined']]) {
    const request = { at: at('value-read', marker), properties: false };
    const nativeRead = await typeOracle(config('value-read'), { ...request, question: 'expression' });
    assert.deepEqual([nativeRead.summary, nativeRead.errorType], [summary, false], marker);
    const symbol = await typeOracle(config('value-read'), { ...request, question: 'value' });
    assert.deepEqual([symbol.summary, symbol.errorType], [declared, false], marker);
    const answer = await typeOracle(config('value-read'), request);
    assert.deepEqual([answer.kind, answer.summary, answer.errorType], ['value', summary, false], marker);
    const hook = await compilerHookOracle(config('value-read'), request);
    assert.deepEqual([hook.kind, hook.summary, hook.errorType], ['value', summary, false], marker);
    const hookSymbol = await compilerHookOracle(config('value-read'), { ...request, question: 'value' });
    assert.deepEqual([hookSymbol.summary, hookSymbol.errorType], [declared, false], marker);
  }
});

test('ordinary assignment name anchors ask the member value question for exact literal and identifier spellings', async () => {
  for (const [marker, summary] of [['identifier', 'number'], ['quoted', 'string'], ['numeric', 'boolean'],
    ['hex', 'string'], ['empty', 'number'], ['escaped', 'boolean']]) {
    const request = { at: at('property-selection', marker) };
    const control = await typeOracle(config('property-selection'), { ...request, question: 'value' });
    assert.deepEqual([control.summary, control.errorType], [summary, false], marker);
    const answer = await typeOracle(config('property-selection'), request);
    assert.deepEqual([answer.kind, answer.summary, answer.errorType], ['value', summary, false], marker);
    const hook = await compilerHookOracle(config('property-selection'), request);
    assert.deepEqual([hook.kind, hook.summary, hook.errorType], ['value', summary, false], marker);
  }
});

test('module-specifier literals have no Q4 expression anchor in import export or import-equals', async () => {
  for (const marker of ['moduleSpecifier', 'exportModule', 'equalsModule']) {
    const request = { at: at('property-selection', marker) };
    await assert.rejects(typeOracle(config('property-selection'), request), /UnsupportedTarget/);
    await assert.rejects(compilerHookOracle(config('property-selection'), request), /UnsupportedTarget/);
  }
});
function textAt(fixture, text, token, file = 'cases.tsx') {
  const source = fs.readFileSync(path.join(fixtures, fixture, file), 'utf8');
  const index = source.indexOf(text);
  assert.notEqual(index, -1, text);
  return { file, ...position(lineStarts(source), index + text.indexOf(token)) };
}
async function generated(t, source, request = {}) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-type-generated-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  fs.writeFileSync(path.join(directory, 'cases.ts'), source);
  fs.writeFileSync(path.join(directory, 'tsconfig.json'), JSON.stringify({ compilerOptions: { strict: true, target: 'esnext', module: 'esnext', noEmit: true }, files: ['cases.ts'] }));
  const offset = source.indexOf('/*#query*/') + '/*#query*/'.length;
  return compilerHookOracle(path.join(directory, 'tsconfig.json'), { ...request, at: { file: 'cases.ts', ...position(lineStarts(source), offset) } });
}

test('literal relation-depth boundaries retain consumed overflow False separately from ordinary False', async t => {
  for (const [depth, maximum, summary, failures, diagnostics] of [
    [99, 99, 'true', [], []], [100, 100, 'false', ['relation_depth_limit'], [2321]],
    [101, 100, 'false', ['relation_depth_limit'], [2321]],
  ]) {
    const answer = await generated(t, relationDepthSource(depth));
    assert.deepEqual([answer.summary, answer.errorType], [summary, false], depth);
    assert.equal(answer.beforeSummary.counters.relationSourceDepth, maximum, depth);
    assert.equal(answer.beforeSummary.counters.relationTargetDepth, maximum, depth);
    assert.deepEqual(answer.beforeSummary.demandFailures, failures, depth);
    assert.deepEqual(answer.beforeSummary.diagnosticCodes, diagnostics, depth);
  }
  const incompatible = await generated(t, relationDepthSource(99, 'different'));
  assert.deepEqual([incompatible.summary, incompatible.errorType], ['false', false]);
  assert.deepEqual(incompatible.beforeSummary.demandFailures, []);
  assert.deepEqual(incompatible.beforeSummary.diagnosticCodes, []);
});

test('natural subtype estimates use the strict threshold and retain an error array child', async t => {
  for (const [removals, processed, estimate, constituents, invalid] of [
    [43, 126, 991250, 1207, false], [42, 125, 1000000, 1208, false], [41, 124, 1007500, 1, true],
  ]) {
    const answer = await generated(t, subtypeRemovalSource(1250, removals), { inspectArrayElement: true });
    assert.equal(answer.errorType, false, removals);
    assert.deepEqual(answer.arrayElement, { errorType: invalid, constituents }, removals);
    assert.deepEqual(answer.beforeSummary.observations.subtypeEstimate, { count: 100000, length: 1250, processed, estimate }, removals);
    assert.deepEqual(answer.beforeSummary.demandFailures, invalid ? ['subtype_complexity_limit'] : [], removals);
    assert.deepEqual(answer.beforeSummary.diagnosticCodes, invalid ? [2590] : [], removals);
    if (invalid) assert.equal(answer.summary, 'any[]');
  }
});

test('native active-key and deeply-nested Maybe protocols preserve admitted and rejected final relations', async t => {
  for (const [mode, branch, depth, summary, result, cacheSize] of [
    ['active', 'activeKeyMaybe', 1, 'true', 3, 1], ['active-false', 'activeKeyMaybe', 1, 'false', 0, 1],
    ['deep', 'deeplyNestedMaybe', 3, 'true', 3, 3],
  ]) {
    const answer = await generated(t, relationMaybeSource(mode));
    assert.deepEqual([answer.summary, answer.errorType], [summary, false], mode);
    assert.deepEqual(answer.beforeSummary.observations?.[branch], { sourceDepth: depth, targetDepth: depth, allowance: 2000000 }, mode);
    assert.deepEqual(answer.beforeSummary.observations.relationFirstFinish,
      { allowance: 2000000 - cacheSize, cacheSize, result, overflow: false }, mode);
    assert.deepEqual(answer.beforeSummary.demandFailures, [], mode);
    assert.deepEqual(answer.beforeSummary.diagnosticCodes, [], mode);
  }
});

test('natural relation allowance distinguishes guard exhaustion from unwinding cache commits', async t => {
  for (const [length, used, allowance, cacheSize, summary, invalid] of [
    [1998, 1999996, 1, 1999999, 'true', false],
    [1999, 2000000, -2, 2000002, 'false', true], [2000, 2000000, -2, 2000002, 'false', true],
  ]) {
    const answer = await generated(t, relationAllowanceSource(length));
    assert.deepEqual([answer.summary, answer.errorType], [summary, false], length);
    assert.equal(answer.beforeSummary.counters.relationInitialAllowance, 2000000, length);
    assert.equal(answer.beforeSummary.counters.relationAllowanceUsed, used, length);
    const finish = answer.beforeSummary.observations.relationFirstFinish;
    assert.deepEqual([finish.allowance, finish.cacheSize, finish.overflow], [allowance, cacheSize, invalid], length);
    assert.deepEqual(answer.beforeSummary.demandFailures, invalid ? ['relation_complexity_limit'] : [], length);
    assert.deepEqual(answer.beforeSummary.diagnosticCodes, invalid ? [2859] : [], length);
  }
});

test('an unresolved selected member seed remains a failure', async t => {
  await assert.rejects(generated(t, 'export {};\n/*#query*/missing;\n', { operation: 'member' }), /UnresolvedTarget/);
});

test('each parameter-property producing modifier exposes only its first member anchor', async () => {
  const fixture = 'parameter-properties';
  for (const marker of ['public', 'private', 'protected', 'readonly', 'override', 'mixed']) {
    const answer = await compilerHookOracle(config(fixture), { operation: 'member', at: at(fixture, marker) });
    assert.equal(answer.selection.syntax, 'parameterPropertyModifier');
    assert.equal(answer.selection.key, 'foo');
    assert.equal(answer.rawSymbolFlags, 4);
    assert.equal(answer.rawOrigins[0].location.start.line, at(fixture, `${marker}Name`).line);
    const local = answer.candidateObservations.find(row => row.location.start.line === at(fixture, `${marker}Name`).line
      && row.location.start.column === at(fixture, `${marker}Name`).column);
    assert.equal(local.symbolFlags, 1);
  }
  await assert.rejects(compilerHookOracle(config(fixture), { operation: 'member', at: at(fixture, 'secondary') }), /UnsupportedTarget/);
});

test('member candidate ledger accounts for decoded names and syntax roles before native matching', async () => {
  const fixture = 'candidate-inventory';
  const answer = await compilerHookOracle(config(fixture), { operation: 'member', at: at(fixture, 'fooSeed', 'cases.tsx') });
  assert.ok(Array.isArray(answer.candidateLedger), 'decoded candidate ledger is required');
  const starts = rows => rows.map(row => ({ file: row.location.file, ...row.location.start }));
  const expected = ['fooSeed', 'fooLocal', 'fooAssignment', 'fooShorthand', 'fooModifier', 'fooParameter', 'fooParameterUse',
    'fooComputedLiteral', 'fooComputedVariable', 'fooElement', 'fooComputedArgument', 'fooEscaped', 'fooProps', 'fooTagDeclaration']
    .map(marker => at(fixture, marker, 'cases.tsx'));
  expected.push(textAt(fixture, '<Panel foo=', 'foo'), textAt(fixture, '<tags.foo ', 'foo'), textAt(fixture, '<tags.foo foo=', 'foo='),
    textAt(fixture, '"foo"', '"foo"', 'keys.json'));
  expected.sort((a, b) => a.file < b.file ? -1 : a.file > b.file ? 1 : a.line - b.line || a.column - b.column);
  assert.deepEqual(starts(answer.candidateLedger), expected);
  assert.deepEqual(starts(answer.candidateDemands.map(location => ({ location }))), expected);
  assert.equal(answer.schedule.candidates, expected.length);
  const shorthand = answer.candidateLedger.find(row => row.location.start.line === at(fixture, 'fooShorthand', 'cases.tsx').line);
  assert.deepEqual(shorthand.roles, ['declaration', 'use']);
  assert.equal(answer.candidateObservations.length, expected.length);
  assert.ok(answer.candidateObservations.every(row => row.meaning === 1));
  const observedShorthand = answer.candidateObservations.find(row => row.location.start.line === at(fixture, 'fooShorthand', 'cases.tsx').line);
  assert.deepEqual(observedShorthand.origins.map(origin => origin.kind), ['ShorthandPropertyAssignment']);
  assert.deepEqual(observedShorthand.lexicalOrigins.map(origin => origin.kind), ['VariableDeclaration']);
  for (const marker of ['fooComputedVariable', 'fooComputedArgument']) {
    const observed = answer.candidateObservations.find(row => row.location.start.line === at(fixture, marker, 'cases.tsx').line);
    assert.deepEqual(observed.relatedOrigins, [], 'computed variables keep lexical identity');
  }
});

test('decoded candidate observations preserve raw scan misses and seed role distinctions', async () => {
  const fixture = 'candidate-inventory';
  for (const [seed, use, key, count] of [['decodedSeed', 'decodedElement', 'decoded', 5], ['numericSeed', 'numericElement', '16', 4],
    ['emptySeed', 'emptyElement', '', 4], ['privateSeed', 'privateUse', '#foo', 2]]) {
    const answer = await compilerHookOracle(config(fixture), { operation: 'member', at: at(fixture, seed, 'cases.tsx') });
    assert.equal(answer.selection?.key, key, 'selected decoded key is required');
    assert.equal(answer.candidateLedger.length, count);
    assert.ok(answer.candidateLedger.every(candidate => candidate.key === key));
    assert.equal(answer.candidateDemands.length, count);
    const observed = answer.candidateObservations.find(candidate => candidate.location.start.line === at(fixture, use, 'cases.tsx').line);
    assert.deepEqual(observed.relatedOrigins, answer.rawOrigins);
    assert.equal(observed.validRaw, true);
    assert.equal(observed.rawTextMatch, seed === 'privateSeed');
    const rawOccurrences = answer.rawGroups.flatMap(group => group.occurrences);
    assert.equal(rawOccurrences.some(location => location.start.line === observed.location.start.line), seed === 'privateSeed');
  }
  const parameter = await compilerHookOracle(config(fixture), { operation: 'member', at: at(fixture, 'fooParameter', 'cases.tsx') });
  const member = await compilerHookOracle(config(fixture), { operation: 'member', at: at(fixture, 'fooModifier', 'cases.tsx') });
  assert.equal(parameter.rawSymbolFlags, 1);
  assert.equal(member.rawSymbolFlags, 4);
  assert.equal(parameter.selection.syntax, 'parameterPropertyName');
  assert.equal(member.selection.syntax, 'parameterPropertyModifier');
  const shorthand = await compilerHookOracle(config(fixture), { operation: 'member', at: at(fixture, 'fooShorthand', 'cases.tsx') });
  assert.deepEqual(shorthand.rawOrigins.map(origin => origin.kind), ['VariableDeclaration']);
  assert.equal(shorthand.rawOrigins[0].location.start.line, at(fixture, 'fooLocal', 'cases.tsx').line);
  const jsonAt = textAt(fixture, '"foo"', '"foo"', 'keys.json');
  const json = await compilerHookOracle(config(fixture), { operation: 'member', at: jsonAt });
  assert.equal(json.rawOrigins[0].location.file, 'keys.json');
  await assert.rejects(typeOracle(config(fixture), { at: jsonAt }), /Q4 positions inside JSON/);
  await assert.rejects(memberOracle(config(fixture), { at: at(fixture, 'fooModifier', 'cases.tsx') }), /symbol pair requires the compiler hook/);
});

test('namespaced JSX attribute candidates retain the exact key and native shape rejection despite raw scan misses', async () => {
  const fixture = 'candidate-inventory';
  const answer = await compilerHookOracle(config(fixture), { operation: 'member', at: at(fixture, 'namespacedSeed', 'cases.tsx') });
  assert.equal(answer.candidateLedger.length, 2, 'namespaced attribute must be accounted for');
  assert.deepEqual(answer.candidateLedger.map(row => row.key), ['ns:foo', 'ns:foo']);
  const observed = answer.candidateObservations.find(row => row.location.start.line === 38);
  assert.ok(observed);
  assert.equal(observed.validRaw, false);
  assert.equal(observed.rawTextMatch, false);
  // Native isValidReferencePosition has no JsxNamespacedName branch. The
  // decoded inventory retains the compound key, but never bypasses that gate.
  assert.deepEqual(observed.origins, []);
  assert.deepEqual(observed.relatedOrigins, []);
  assert.ok(answer.candidateDemands.every(row => row.start.line !== 38));
});

test('fresh request resolves enclosing calls before rejected-overload callback parameters', async () => {
  const answer = await typeOracle(config('overload-context'), { at: at('overload-context', 'parameter') });
  assert.equal(answer.summary, 'string');
  assert.equal(answer.kind, 'value');
  assert.deepEqual(answer.prelude.map(call => call.kind), ['CallExpression']);
});

test('cold and diagnostics-first observations remain distinct from the defined prelude', async () => {
  for (const history of ['cold', 'diagnostics']) {
    const answer = await typeOracle(config('overload-context'), { at: at('overload-context', 'parameter'), history });
    assert.equal(answer.summary, 'any', history);
    assert.deepEqual(answer.prelude, [], history);
  }
  const member = await typeOracle(config('overload-context'), { at: at('overload-context', 'member') });
  assert.equal(member.kind, 'expression');
  assert.equal(member.summary, '() => string');
  assert.deepEqual(member.origins.map(origin => origin.location.start.line), [5]);
  const result = await typeOracle(config('overload-context'), { at: at('overload-context', 'result') });
  assert.equal(result.summary, '"second"');
  // This repeated request must begin fresh rather than inherit the result query.
  assert.equal((await typeOracle(config('overload-context'), { at: at('overload-context', 'parameter'), history: 'cold' })).summary, 'any');
});

test('selection distinguishes declared, value, expression and containing type questions', async () => {
  const inspect = marker => typeOracle(config('selection'), { at: at('selection', marker) });
  const declared = await inspect('class');
  assert.deepEqual([declared.kind, declared.summary], ['declared', 'Widget']);
  const value = await inspect('classUse');
  assert.deepEqual([value.kind, value.summary], ['expression', 'typeof Widget']);
  const property = await inspect('access');
  assert.deepEqual([property.kind, property.summary], ['expression', 'number']);
  const literal = await inspect('key');
  assert.deepEqual([literal.kind, literal.summary], ['expression', '"n"']);
  const reference = await inspect('type');
  assert.deepEqual([reference.kind, reference.summary], ['type', 'Box<string>']);
  assert.deepEqual((await inspect('alias')).properties.map(property => [property.name, property.summary]),
    [['value', 'string'], ['next', 'Box<string> | undefined']]);
});

test('non-call context and recursive aliases are callable in a fresh checker', async () => {
  const annotated = await typeOracle(config('selection'), { at: at('selection', 'contextParameter') });
  assert.equal(annotated.summary, 'string');
  assert.deepEqual(annotated.prelude, []);
  const satisfied = await typeOracle(config('selection'), { at: at('selection', 'satisfiesParameter'), diagnostics: true });
  assert.equal(satisfied.summary, 'number');
  assert.equal(satisfied.errorType, false);
  assert.ok(satisfied.diagnostics.some(diagnostic => diagnostic.code === 2322));
  const demandOnly = await typeOracle(config('selection'), { at: at('selection', 'satisfiesParameter') });
  assert.deepEqual(demandOnly.diagnostics, []);
});

test('compiler hook replays inherited configuration and package conditional exports', async t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-type-replay-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  fs.cpSync(path.join(fixtures, 'input-replay'), directory, { recursive: true });
  fs.mkdirSync(path.join(directory, 'node_modules'), { recursive: true });
  fs.renameSync(path.join(directory, 'dual'), path.join(directory, 'node_modules', 'dual'));
  const sourceFile = path.join(directory, 'cases.mts');
  fs.writeFileSync(sourceFile, Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), fs.readFileSync(sourceFile),
    Buffer.from('export const container = { /*#property*/field: 1 };\ncontainer./*#memberUse*/field;\n')]));
  const request = { at: at('input-replay', 'query', 'cases.mts') };
  const leaf = path.join(directory, 'tsconfig.json');
  const native = await typeOracle(leaf, request);
  assert.equal(native.summary, '"import"');
  const replay = await compilerHookOracle(leaf, request);
  assert.equal(replay.summary, native.summary);
  assert.equal(replay.compilerOptions.module, native.compilerOptions.module);
  assert.ok(replay.inputFiles.some(file => file.endsWith('/base.json')));
  assert.ok(replay.inputFiles.some(file => file.endsWith('/node_modules/dual/package.json')));
  const text = fs.readFileSync(sourceFile, 'utf8');
  const memberAt = { file: 'cases.mts', ...position(lineStarts(text), text.indexOf('/*#memberUse*/') + '/*#memberUse*/'.length) };
  const nativeMember = await memberOracle(leaf, { at: memberAt });
  const replayMember = await compilerHookOracle(leaf, { at: memberAt, operation: 'member' });
  assert.deepEqual(replayMember.rawOrigins, nativeMember.rawOrigins);
  assert.deepEqual(replayMember.candidateDemands, nativeMember.candidates.map(candidate => candidate.location));
  assert.ok(Array.isArray(replay.inputHashes), 'frozen input hashes must cover metadata and BOM source');
  assert.deepEqual(replay.inputHashes.map(row => row.file), replay.inputFiles);
  for (const file of [leaf, path.join(directory, 'base.json'), sourceFile, path.join(directory, 'node_modules', 'dual', 'package.json')]) {
    const row = replay.inputHashes.find(row => row.file === fs.realpathSync(file));
    assert.ok(row, file);
    assert.equal(row.sha256, createHash('sha256').update(fs.readFileSync(file)).digest('hex'), file);
  }
});

test('late-member observations retain raw and combined origins without claiming LS checker sharing', async () => {
  const named = await memberOracle(config('late-members'), { at: at('late-members', 'named') });
  assert.deepEqual(named.rawOrigins.map(origin => origin.location.start.line), [4]);
  assert.deepEqual(named.combinedOrigins.map(origin => origin.location.start.line), [3, 4]);
  const use = await memberOracle(config('late-members'), { at: at('late-members', 'use') });
  assert.deepEqual(use.combinedOrigins, named.combinedOrigins);
  assert.deepEqual(named.blockers, ['public_reference_api_uses_separate_checker']);
  assert.equal(named.normalization.sharedCheckerSchedule, 'unproved');
  // Both exact 'value' literals stay in the independent inventory; native
  // position validity is a later question, separate from decoded name matching.
  assert.deepEqual(named.candidates.map(candidate => candidate.location.start.line), [2, 2, 4, 5]);
});

test('pinned compiler hook normalizes late seeds before searching in one request checker', async () => {
  const named = await compilerHookOracle(config('late-members'), { at: at('late-members', 'named'), operation: 'member' });
  assert.equal(named.status, 'observed');
  assert.deepEqual(named.combinedOrigins.map(origin => origin.location.start.line), [3, 4]);
  assert.deepEqual(named.roots.map(root => root.map(origin => origin.location.start.line)), [[3, 4]]);
  const use = await compilerHookOracle(config('late-members'), { at: at('late-members', 'use'), operation: 'member' });
  assert.deepEqual(use.groups, named.groups);
  assert.equal(named.schedule.checker, 'one');
  const literals = named.candidateObservations.filter(row => row.location.start.line === 2);
  assert.equal(literals.length, 2);
  assert.ok(literals.every(row => !row.validRaw && row.origins.length === 0 && row.relatedOrigins.length === 0));
  assert.ok(named.candidateDemands.every(row => row.start.line !== 2));
  assert.equal(named.schedule.relationshipBeforeCandidates, true);
  assert.equal(named.schedule.searchSetCount, 1);
  assert.deepEqual(named.candidateDemands.map(location => location.start.line), [4, 5]);
});

test('compiler hook preserves contextual-member references in rejected-overload calls', async () => {
  const answer = await compilerHookOracle(config('overload-context'), { at: at('overload-context', 'member'), operation: 'member' });
  assert.deepEqual(answer.groups.map(group => group.occurrences.map(occurrence => occurrence.start.line)), [[5, 8]]);
  assert.deepEqual(answer.roots.map(root => root.map(origin => origin.location.start.line)), [[5]]);
  assert.equal(answer.prelude[0].kind, 'CallExpression');
});

test('compiler conditional-tail boundaries retain final root failure and exact limit events', async () => {
  for (const [marker, tail, errorType, failures] of [
    ['tailUnder', 999, false, []], ['tailAt', 1000, true, ['conditional_tail_limit']], ['tailOver', 1000, true, ['conditional_tail_limit']],
  ]) {
    const answer = await compilerHookOracle(config('compiler-limits'), { at: at('compiler-limits', marker) });
    assert.equal(answer.errorType, errorType, marker);
    assert.equal(answer.counters.conditionalTail, tail, marker);
    assert.deepEqual(answer.demandFailures, failures, marker);
  }
});

test('compiler cross-product boundaries use exact operation units and final type results', async t => {
  for (const [left, right, size, errorType] of [[3, 33333, 99999, false], [100, 1000, 100000, true], [11, 9091, 100001, true]]) {
    const answer = await generated(t, crossProductSource(left, right));
    assert.equal(answer.counters.crossProductUnion, size);
    assert.equal(answer.errorType, errorType);
    assert.deepEqual(answer.demandFailures, errorType ? ['union_complexity_limit'] : []);
  }
});

test('a non-error tuple root retains a demanded instantiation failure', async t => {
  const before = await generated(t, headRecursiveSource(47));
  assert.equal(before.counters.instantiationDepth, 99);
  assert.deepEqual(before.demandFailures, []);
  assert.equal(before.beforeSummary?.counters.instantiationDepth, 99);
  assert.deepEqual(before.beforeSummary.demandFailures, []);
  for (const length of [48, 49]) {
    const recovery = await generated(t, headRecursiveSource(length));
    assert.equal(recovery.counters.instantiationDepth, 100);
    assert.equal(recovery.errorType, false);
    assert.deepEqual(recovery.demandFailures, ['instantiation_limit']);
    assert.equal(recovery.beforeSummary.counters.instantiationDepth, 100);
    assert.deepEqual(recovery.beforeSummary.demandFailures, ['instantiation_limit']);
    assert.match(recovery.summary, /^\[0, 0,/);
  }
});

test('natural instantiation-count boundaries occur before summary construction', async t => {
  for (const [identities, count, errorType] of [[2, 4999999, false], [3, 5000000, true], [4, 5000000, true]]) {
    const answer = await generated(t, instantiationCountSource(identities));
    assert.equal(answer.counters.instantiationCount, count, identities);
    assert.equal(answer.errorType, errorType, identities);
    const failures = errorType ? ['instantiation_limit'] : [];
    assert.deepEqual(answer.demandFailures, failures, identities);
    assert.equal(answer.beforeSummary?.counters.instantiationCount, count, identities);
    assert.deepEqual(answer.beforeSummary.demandFailures, failures, identities);
    assert.equal(answer.summary, errorType ? 'any' : '2 | 26 | 885 | 900 | 901 | 902 | 903 | 904 | 905 | 906 | 907 | 908 | 909 | 910');
  }
});

test('call observations include wrapper normalization and preserve direct-target exclusions', async () => {
  const answer = await compilerHookOracle(config('call-targets'), { at: at('call-targets', 'declaration'), operation: 'member' });
  assert.ok(Array.isArray(answer.callObservations));
  for (const [marker, expected] of [['paren', true], ['nonNull', true], ['angle', true], ['as', true], ['satisfies', true],
    ['optional', true], ['constructor', true], ['tag', true], ['argument', false], ['read', false], ['callProperty', false],
    ['implicitDecorator', false], ['explicitDecorator', true]]) {
    const anchor = at('call-targets', marker);
    const observation = answer.callObservations.find(row => row.location.start.line === anchor.line && row.location.start.column === anchor.column);
    assert.ok(observation, marker);
    assert.equal(observation.call, expected, marker);
  }
  for (const [marker, expected] of [['namespaceMember', true], ['namespaceReceiver', false]]) {
    const namespace = await compilerHookOracle(config('call-targets'), { at: at('call-targets', marker), operation: 'member' });
    const anchor = at('call-targets', marker);
    const observation = namespace.callObservations.find(row => row.location.start.line === anchor.line && row.location.start.column === anchor.column);
    assert.ok(observation, marker);
    assert.equal(observation.call, expected, marker);
  }
});

test('same-checker candidate demands preserve exact relative-path and native UTF-8 filename order', async t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-type-order-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const root = path.join(directory, 'project');
  fs.mkdirSync(root);
  const declaration = '/* 😀 */ export interface Holder { /*#query*/method(): void; }\n';
  fs.writeFileSync(path.join(directory, 'zoutside.ts'), declaration);
  const files = ['a.ts', '\u{10000}.ts', '\ue000.ts'];
  for (const file of files) fs.writeFileSync(path.join(root, file), 'import type { Holder } from "../zoutside";\ndeclare const value: Holder;\nvalue.method;\n');
  fs.writeFileSync(path.join(root, 'tsconfig.json'), JSON.stringify({ compilerOptions: { strict: true, noEmit: true }, files: [...files, '../zoutside.ts'] }));
  const offset = declaration.indexOf('/*#query*/') + '/*#query*/'.length;
  const answer = await compilerHookOracle(path.join(root, 'tsconfig.json'), { operation: 'member',
    at: { file: '../zoutside.ts', ...position(lineStarts(declaration), offset) } });
  assert.ok(Array.isArray(answer.candidateDemands));
  // Pinned CompareStringsCaseSensitive uses strings.Compare: BMP U+E000
  // precedes astral U+10000, matching final Rust Location Ord.
  const expected = ['../zoutside.ts', 'a.ts', '\ue000.ts', '\u{10000}.ts'];
  assert.deepEqual(answer.candidateDemands.map(location => location.file), expected);
  const start = position(lineStarts(declaration), offset);
  assert.deepEqual(answer.candidateDemands[0].start, start);
  assert.deepEqual(answer.candidateDemands[0].end, { line: start.line, column: start.column + 'method'.length });
});

test('compiler hook rejects unimplemented or unknown checking histories explicitly', async () => {
  for (const [history, message] of [['diagnostics', /Unsupported compiler hook history/], ['invented', /Unknown checking history/]]) {
    await assert.rejects(compilerHookOracle(config('selection'), { at: at('selection', 'class'), history }), message);
  }
  await assert.rejects(compilerHookOracle(config('selection'), { at: at('selection', 'class'), operation: 'invented' }), /Unknown oracle operation/);
});

test('CLI delivers the pinned compiler result and fails setup/version errors without a result', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-type-cli-'));
  try {
    const request = path.join(directory, 'request.json');
    fs.writeFileSync(request, JSON.stringify({ config: config('overload-context'), at: at('overload-context', 'parameter') }));
    const cli = fileURLToPath(new URL('./type-oracle.mjs', import.meta.url));
    const valid = spawnSync(process.execPath, [cli, request], { encoding: 'utf8' });
    assert.equal(valid.status, 0, valid.stderr);
    assert.equal(JSON.parse(valid.stdout).summary, 'string');
    const missing = spawnSync(process.execPath, [cli, request], { encoding: 'utf8', env: { ...process.env, TYPE_SCRIPT_NATIVE: '' } });
    assert.equal(missing.status, 2);
    assert.equal(missing.stdout, '');
    assert.match(missing.stderr, /TYPE_SCRIPT_NATIVE must name the TypeScript 7\.0\.2 native compiler/);
    fs.writeFileSync(path.join(directory, 'package.json'), '{"version":"0.0.0"}');
    const wrong = spawnSync(process.execPath, [cli, request], { encoding: 'utf8', env: { ...process.env, TYPE_SCRIPT_SEMANTIC_API: directory } });
    assert.equal(wrong.status, 2);
    assert.equal(wrong.stdout, '');
    assert.match(wrong.stderr, /Expected semantic API 7\.0\.2, received 0\.0\.0/);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

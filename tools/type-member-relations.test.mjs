// Executable compiler baseline, not assertions about the native Rust checker.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compilerHookOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/member-relations/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'cases.ts'), 'utf8');
function at(marker) {
  const text = `/*#${marker}*/`;
  const start = source.indexOf(text);
  assert.notEqual(start, -1, marker);
  assert.equal(source.indexOf(text, start + 1), -1, `unique ${marker}`);
  return { file: 'cases.ts', ...position(lineStarts(source), start + text.length) };
}
const observe = marker => compilerHookOracle(path.join(directory, 'tsconfig.json'), { operation: 'member', at: at(marker) });
const origins = rows => rows.map(row => row.location.start.line);
const groups = answer => answer.groups.map(group => ({ origins: origins(group.origins), occurrences: group.occurrences.map(row => row.start.line) }));

test('generic, homomorphic mapped and ordinary spread uses retain the declared member root', async () => {
  const expected = [{ origins: [2], occurrences: [2, 5, 6, 8, 10, 12, 14, 18, 20] }];
  for (const marker of ['generic', 'stringUse', 'numberUse', 'partial', 'readonly', 'picked', 'omitted', 'ordinarySpread', 'overlapSpread']) {
    const answer = await observe(marker);
    assert.deepEqual(answer.roots.map(origins), [[2]], marker);
    assert.deepEqual(groups(answer), expected, marker);
    assert.deepEqual(answer.demandFailures, [], marker);
  }
  const remapped = await observe('remapped');
  assert.deepEqual(remapped.rawOrigins, [], 'key remapping creates a declarationless member');
  assert.deepEqual(remapped.roots.map(origins), [[2]]);
  assert.deepEqual(groups(remapped), [{ origins: [2], occurrences: [16] }]);
});

test('accessor and readonly changing spreads preserve native synthetic roots', async () => {
  for (const [marker, root, occurrences] of [
    ['getSpread', 21, [21, 23]], ['setSpread', 24, [24, 26]], ['readonlySpread', 27, [27, 29]],
  ]) {
    const answer = await observe(marker);
    assert.deepEqual(answer.roots.map(origins), [[root]], marker);
    assert.deepEqual(groups(answer), [{ origins: [root], occurrences }], marker);
    assert.deepEqual(answer.demandFailures, [], marker);
  }
});

test('contextual relationships retain declaration and use direction instead of structural merging', async () => {
  const contextual = [{ origins: [30], occurrences: [30, 31, 32, 34] }];
  for (const marker of ['host', 'annotated', 'annotatedUse']) assert.deepEqual(groups(await observe(marker)), contextual, marker);
  assert.deepEqual(groups(await observe('satisfied')), [...contextual, { origins: [32], occurrences: [35] }]);
  assert.deepEqual(groups(await observe('satisfiedUse')), [{ origins: [32], occurrences: [32, 35] }]);
  for (const marker of ['unrelated', 'unrelatedUse'])
    assert.deepEqual(groups(await observe(marker)), [{ origins: [33], occurrences: [33, 36] }], marker);
});

test('explicit base and implemented-interface relationships do not connect unrelated equal signatures', async () => {
  const hierarchy = [{ origins: [37], occurrences: [37, 41] }, { origins: [38], occurrences: [38, 42] }];
  for (const marker of ['base', 'override', 'baseUse', 'overrideUse']) assert.deepEqual(groups(await observe(marker)), hierarchy, marker);
  const implementation = [{ origins: [39], occurrences: [39] }, { origins: [40], occurrences: [40, 43] }];
  for (const marker of ['interface', 'implements', 'implementsUse']) assert.deepEqual(groups(await observe(marker)), implementation, marker);
});

function namedOrigin(marker, kind, length = 4) {
  const { file, line, column } = at(marker);
  return { kind, location: { file, start: { line, column }, end: { line, column: column + length } } };
}
const referenceLocation = (marker, length) => {
  const { file, line, column } = at(marker);
  return { file, start: { line, column }, end: { line, column: column + length } };
};

test('static, instance, private and accessor identities retain exact origins at shared lines', async () => {
  for (const [declaration, use, kind] of [
    ['static', 'staticUse', 'MethodDeclaration'], ['instance', 'instanceUse', 'MethodDeclaration'],
    ['privateFirst', 'privateFirstUse', 'PropertyDeclaration'], ['privateSecond', 'privateSecondUse', 'PropertyDeclaration'],
  ]) {
    const expected = [{ origins: [namedOrigin(declaration, kind)], occurrences: [referenceLocation(declaration, 4), referenceLocation(use, 4)] }];
    const declared = await observe(declaration);
    const used = await observe(use);
    assert.deepEqual(declared.groups, expected, declaration);
    assert.deepEqual(used.groups, expected, use);
    assert.deepEqual(declared.roots, [[namedOrigin(declaration, kind)]], declaration);
  }
  const expected = [{ origins: [namedOrigin('getter', 'GetAccessor'), namedOrigin('setter', 'SetAccessor')],
    occurrences: [referenceLocation('getter', 4), referenceLocation('setter', 4), referenceLocation('accessorUse', 4)] }];
  for (const marker of ['getter', 'setter', 'accessorUse']) assert.deepEqual((await observe(marker)).groups, expected, marker);
});

test('inline union substitution and interface union seeds preserve their distinct directed groups', async () => {
  const left = namedOrigin('inlineLeft', 'PropertySignature');
  const right = namedOrigin('inlineRight', 'PropertySignature');
  const inline = [{ origins: [left], occurrences: [referenceLocation('inlineLeft', 4)] },
    { origins: [left, right], occurrences: [referenceLocation('inlineUse', 4)] },
    { origins: [right], occurrences: [referenceLocation('inlineRight', 4)] }];
  for (const marker of ['inlineLeft', 'inlineRight', 'inlineUse']) assert.deepEqual((await observe(marker)).groups, inline, marker);
  const interfaceLeft = [{ origins: [48], occurrences: [48] }, { origins: [48, 49], occurrences: [51] }];
  const interfaceRight = [{ origins: [48, 49], occurrences: [51] }, { origins: [49], occurrences: [49] }];
  assert.deepEqual(groups(await observe('interfaceLeft')), interfaceLeft);
  assert.deepEqual(groups(await observe('interfaceRight')), interfaceRight);
  assert.deepEqual(groups(await observe('interfaceUnionUse')), [{ origins: [48], occurrences: [48] },
    { origins: [48, 49], occurrences: [51] }, { origins: [49], occurrences: [49] }]);
});

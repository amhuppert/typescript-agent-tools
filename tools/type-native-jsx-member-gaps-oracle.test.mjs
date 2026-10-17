import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';

// JSX attribute uses of a props member are compiler reference candidates
// related through the contextual props type. The product defers that JSX
// typing, so these exact candidates must become jsx_typing_required gaps.
const root = fileURLToPath(new URL('../fixtures/types/native-jsx-member-gaps/', import.meta.url));
function at(file, marker) {
  const source = fs.readFileSync(path.join(root, file), 'utf8'), tag = `/*#${marker}*/`;
  const offset = source.indexOf(tag);
  assert.notEqual(offset, -1, marker);
  return {file, ...position(lineStarts(source), offset + tag.length)};
}
function observation(answer, target) {
  return answer.candidateObservations.find(row => row.location.file === target.file
    && row.location.start.line === target.line && row.location.start.column === target.column);
}

test('JSX attribute candidates relate to the props member only through JSX typing', async () => {
  const answer = await compilerHookOracle(path.join(root, 'tsconfig.json'), {operation: 'member', at: at('comp.tsx', 'prop')});
  fs.writeFileSync('.cc/temp/native-jsx-member-gaps.json', JSON.stringify(answer, null, 2) + '\n');
  assert.equal(answer.compiler, '7.0.2');
  assert.deepEqual(answer.demandFailures, []);
  for (const marker of ['attrFirst', 'attrSecond']) {
    const row = observation(answer, at('use.tsx', marker));
    assert.ok(row, marker);
    assert.equal(row.validRaw, true, marker);
    assert.equal(row.relatedOrigins.length, 1, marker);
    assert.equal(row.relatedOrigins[0].location.file, 'comp.tsx', marker);
  }
  const access = observation(answer, at('comp.tsx', 'access'));
  assert.equal(access.relatedOrigins.length, 1);
});

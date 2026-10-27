import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {compilerHookOracle, typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
import {scratchPath} from './harness-scratch.mjs';

const root = fileURLToPath(new URL('../fixtures/types/native-before-use/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
function request(marker) {
  const label = `/*#${marker}*/`;
  return {at: {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(label) + label.length)}, properties: false};
}

test('native before-use preserves diagnostics-only results and actual static-block flow admission', async () => {
  const observed = [];
  for (const config of ['tsconfig.json', 'legacy.json']) for (const [marker, summary] of [
    ['noBlocks', 'number'], ['emptyBlock', 'number'], ['initialized', 'number'],
    ['alreadyOptional', 'number | undefined'], ['optionalInitialized', 'number | undefined'],
    ['anyProperty', 'any'], ['unknownProperty', 'unknown'], ['laterBlock', 'number'],
    ['earlierDeclaration', 'number'], ['staticMethod', '() => number'],
    ['instanceDeferred', 'number'], ['selfReference', 'number'],
  ]) {
    const answer = await compilerHookOracle(path.join(root, config), request(marker));
    assert.equal(answer.errorType, false, marker);
    assert.equal(answer.summary, summary, marker);
    const label = `/*#${marker}*/`;
    const start = source.indexOf(label) + label.length;
    const end = start + source.slice(start).match(/^\w+/)[0].length;
    const classStart = source.lastIndexOf('class ', start);
    const classEnd = source.indexOf('\n}', start) + 2;
    const entries = Object.entries(answer.beforeSummary.observations);
    observed.push({config, marker, before: entries.filter(([key, value]) => key.startsWith('beforeUse:') && value.usageEnd === end).map(([,value]) => value),
      flow: entries.filter(([key, value]) => key.startsWith('staticBlockFlow:') && value.propertyPos >= classStart && value.propertyEnd <= classEnd).map(([,value]) => value)});
    fs.writeFileSync(scratchPath('native-before-use-keyed-observations.json'), JSON.stringify(observed, null, 2));
  }
  fs.writeFileSync(scratchPath('native-before-use-observations.json'), JSON.stringify(observed, null, 2));
  assert.equal(observed.length, 24);
});


test('native loose static-block optional-type assertions remain explicit compiler failures', async () => {
  const failures = [];
  for (const marker of ['emptyBlock', 'initialized', 'alreadyOptional', 'optionalInitialized', 'anyProperty', 'unknownProperty']) {
    await assert.rejects(() => compilerHookOracle(path.join(root, 'loose.json'), request(marker)),
      error => /Debug failure[\s\S]*getOptionalType/.test(error.message), marker);
    let observed;
    try {
      const answer = await typeOracle(path.join(root, 'loose.json'), request(marker));
      observed = {kind:'answer', summary:answer.summary, errorType:answer.errorType};
    } catch (error) {
      observed = {kind:'failure', message:error.message};
    }
    failures.push({marker, ...observed});
    fs.writeFileSync(scratchPath('native-before-use-loose-public.json'), JSON.stringify(failures, null, 2));
    assert.equal(observed.kind, 'failure', marker);
    assert.match(observed.message, /Debug failure[\s\S]*getOptionalType/, marker);
  }
  for (const [marker, summary] of [['noBlocks', 'number'], ['laterBlock', 'number'], ['earlierDeclaration', 'number'],
    ['staticMethod', '() => number'], ['instanceDeferred', 'number'], ['selfReference', 'number']]) {
    const answer = await typeOracle(path.join(root, 'loose.json'), request(marker));
    assert.equal(answer.summary, summary, marker);
    assert.equal(answer.errorType, false, marker);
  }
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { compilerHookOracle } from './oracle.mjs';
import { lineStarts, position } from '../../../tools/semantic-oracle.mjs';
const root = path.resolve('fixtures/types/native-member-values');
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const cases = [
  ...['tsconfig.json', 'exact.json', 'loose.json', 'loose-exact.json'].map(config => [config, 'plain', 'optional']),
  ...['fixed', 'plain', 'getterOnly', 'setterOnly', 'paired', 'fromSetter',
    'ignoredBadSetter', 'lazyRead', 'method', 'overloaded', 'optionalMethod'].map(name => ['tsconfig.json', 'plain', name]),
  ['tsconfig.json', 'generic', 'method'], ['tsconfig.json', 'generic', 'value'],
  ...['protectedValue', 'hiddenValue', 'paired', 'method'].map(name => ['tsconfig.json', 'declared', name]),
];
const rows = [];
for (const [config, variable, memberName] of cases) {
  const prefix = `export declare let ${variable}:`;
  const start = source.indexOf(prefix);
  assert.notEqual(start, -1, variable);
  const offset = start + 'export declare let '.length;
  const request = { at: { file: 'cases.ts', ...position(lineStarts(source), offset) },
    properties: false, memberName };
  const answer = await compilerHookOracle(path.join(root, config), request);
  rows.push({ config, variable, memberName, request, answer });
  fs.writeFileSync(new URL('answers.json', import.meta.url), JSON.stringify(rows, null, 2) + '\n');
  console.log(JSON.stringify({ config, variable, memberName, member:answer.memberValue,
    before:answer.beforeSummary.diagnosticCodes, after:answer.afterMemberValue.diagnosticCodes }));
}

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { compilerHookOracle } from './oracle.mjs';
import { lineStarts, position } from '../../../tools/semantic-oracle.mjs';
const root = path.resolve('fixtures/types/native-member-values');
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
const rows = [];
for (const config of ['exact.json', 'loose.json', 'loose-exact.json']) {
  const offset = source.indexOf('export declare let plain:') + 'export declare let '.length;
  assert.ok(offset >= 'export declare let '.length);
  const request = {at:{file:'cases.ts',...position(lineStarts(source),offset)},
    properties:false,memberName:'optionalMethod'};
  const answer = await compilerHookOracle(path.join(root,config),request);
  rows.push({config,variable:'plain',memberName:'optionalMethod',request,answer});
  fs.writeFileSync(new URL('extra-answers.json',import.meta.url),JSON.stringify(rows,null,2)+'\n');
  const m=answer.memberValue;
  console.log(JSON.stringify({config,read:m.read,nonMissingRead:m.nonMissingRead,write:m.write,
    preflight:answer.observedPreflightDiagnostics,nativeProgram:answer.nativeProgramDiagnosticCodes}));
}

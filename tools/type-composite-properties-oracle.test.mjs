import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts, position} from './semantic-oracle.mjs';
const root = fileURLToPath(new URL('../fixtures/types/native-composite-properties/', import.meta.url));
const source = fs.readFileSync(path.join(root, 'cases.ts'), 'utf8');
for (const config of ['tsconfig.json', 'loose.json', 'exact.json']) {
 test(`native composite property reads, writes and discriminants (${config})`, async () => {
  const rows = [
   ['literalConflict','any',true], ['optionalConflict','number',false],
   ['originalNever','number',false], ['wideConflict','number',false], ['privateConflict','any',true],
   ['union','string | number',false], ['intersection','never',false], ['same','number',false],
   ['optional',config === 'loose.json' ? 'number' : 'number | undefined',false],
   ['required','number',false], ['partial','any',true], ['readonlyWrite','any',true],
   ['intersectionWrite','number',false], ['accessorRead','string | number',false],
   ['accessorWrite','string | number | boolean',false], ['indexed','number',false],
   ['deferred','string | number | boolean',false], ['unhealthy','any',true],
   ['restored',config === 'loose.json' ? 'A' : 'A | undefined',false],
   ['narrowed','{ p: 1; b: number; }',false],
   ['joined','{ p: 0; a: number; } | { p: 1; b: number; }',false],
  ];
  for (const [marker,summary,errorType] of rows) {
   const tag = `/*#${marker}*/`;
   let offset = source.indexOf(tag) + tag.length;
   if (!['restored','narrowed','joined'].includes(marker)) offset = source.indexOf('.p',offset)+1;
   if (['literalConflict','optionalConflict','originalNever','wideConflict','privateConflict'].includes(marker)) offset=source.indexOf('.q',source.indexOf(tag))+1;
   if (marker === 'partial') offset = source.indexOf('.a',source.indexOf(tag))+1;
   const answer = await typeOracle(path.join(root, config), {at:{file:'cases.ts', ...position(lineStarts(source), offset)}, properties:false});
   assert.equal(answer.compiler,'7.0.2');
   assert.equal(answer.summary,summary,marker);
   assert.equal(answer.errorType,errorType,marker);
  }
 });
}

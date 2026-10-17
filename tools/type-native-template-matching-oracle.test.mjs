import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {typeOracle} from './type-oracle.mjs';
import {lineStarts,position} from './semantic-oracle.mjs';
const root=fileURLToPath(new URL('../fixtures/types/native-template-matching/',import.meta.url));
const source=fs.readFileSync(path.join(root,'cases.ts'),'utf8');
export const expected={"numberRound": "12", "numberLeading": "number", "numberRadix": "number", "numberSpace": "number", "numberEmpty": "\"invalid\"", "numberInfinity": "\"invalid\"", "numberZero": "number", "numberExponent": "number", "numberBom": "number", "numberNel": "\"invalid\"", "numberZwsp": "\"invalid\"", "bigRound": "12n", "bigRadix": "bigint", "bigSeparator": "\"invalid\"", "bigSpace": "\"invalid\"", "bigZero": "bigint", "bigLeading": "\"invalid\"", "parts": "[\"left\", \"right\"]", "firstDelimiter": "[\"a\", \"b-c\"]", "unicode": "[\"👋\", \"x\"]", "single": "[\"x\", \"\"]", "genericParts": "[string, `${number}`]", "crossParts": "[`<${number}>`, string]", "numericKey": "\"numeric\"", "badKey": "\"other\"", "upperGood": "\"upper\"", "upperBad": "\"other\"", "nestedGood": "\"nested\"", "nestedBad": "\"other\"", "signedRadix": "\"invalid\"", "numberPlus": "number", "numberBinary": "number", "numberOctal": "number", "numberHugeRadix": "number", "numberOverflowRadix": "\"invalid\"", "numberOverflow": "\"invalid\"", "numberUnderflow": "number", "numberDecimal": "-1.5", "numberSeparator": "\"invalid\"", "numberLeadingEight": "number", "numberNan": "\"invalid\"", "numberNegativeInfinity": "\"invalid\"", "numberBadExponent": "\"invalid\"", "numberArabic": "\"invalid\"", "bigPlus": "\"invalid\"", "bigNegativeRadix": "bigint", "bigBinary": "bigint", "bigOctal": "bigint", "bigTrailing": "\"invalid\"", "bigMinusGap": "\"invalid\"", "bigDoubleZero": "\"invalid\"", "bigEight": "\"invalid\"", "bigFraction": "\"invalid\"", "bigExponent": "\"invalid\"", "bigEmpty": "\"invalid\"", "bigNegative": "-12n", "bigHuge": "9007199254740993123456789n", "bigNegativeLeading": "\"invalid\"", "numberWhitespaceTab": "number", "numberWhitespaceVertical": "number", "numberWhitespaceForm": "number", "numberWhitespaceLf": "number", "numberWhitespaceCr": "number", "numberWhitespaceNbsp": "number", "numberWhitespaceOgham": "number", "numberWhitespaceEnquad": "number", "numberWhitespaceHair": "number", "numberWhitespaceLine": "number", "numberWhitespaceParagraph": "number", "numberWhitespaceNarrow": "number", "numberWhitespaceMath": "number", "numberWhitespaceIdeographic": "number"};
test('native template matching preserves delimiter, numeric and string-mapping decisions',async t=>{
 const results=[];
 for(const config of ['tsconfig.json','loose.json','exact.json']) for(const marker of Object.keys(expected)) {
  const tag=`/*#${marker}*/`;
  const at={file:'cases.ts',...position(lineStarts(source),source.indexOf(tag)+tag.length)};
  const answer=await typeOracle(path.join(root,config),{at});
  results.push({config,marker,summary:answer.summary,error:answer.errorType});
 }
 t.diagnostic(JSON.stringify(results));
 for(const row of results){assert.equal(row.summary,expected[row.marker],`${row.config}/${row.marker}`);assert.equal(row.error,false,`${row.config}/${row.marker}`);}
});

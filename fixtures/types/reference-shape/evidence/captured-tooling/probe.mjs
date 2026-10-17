import fs from 'node:fs';
import path from 'node:path';
import { compilerHookOracle } from './oracle.mjs';
import { lineStarts, position } from '../../../tools/semantic-oracle.mjs';
const here = path.dirname(new URL(import.meta.url).pathname);
const families = {
  object: 'interface Shape<T> { value: T; }',
  call: 'interface Shape<T> { (value: T): T; }',
  construct: 'interface Shape<T> { new(value: T): T; }',
  callMissingIndex: 'interface Shape<T> { (value: T): T; [key: string]: MissingIndex; }',
  objectMissingIndex: 'interface Shape<T> { value: T; [key: string]: MissingIndex; }',
  callConditionalIndex: 'interface Shape<T> { (value: T): T; [key: string]: T extends string ? number : boolean; }',
};
const answers = [];
for (const [family, declaration] of Object.entries(families)) {
  const directory = path.join(here, family); fs.mkdirSync(directory, { recursive: true });
  const source = `${declaration}\nexport let /*#query*/result: Shape<string>;\n`;
  fs.writeFileSync(path.join(directory, 'cases.ts'), source);
  fs.writeFileSync(path.join(directory, 'tsconfig.json'), JSON.stringify({ compilerOptions: { strict: true, noLib: true, noEmit: true }, files: ['cases.ts'] }));
  const request = { at: { file: 'cases.ts', ...position(lineStarts(source), source.indexOf('/*#query*/') + '/*#query*/'.length) } };
  const result = await compilerHookOracle(path.join(directory, 'tsconfig.json'), request);
  answers.push({ family, request, result });
  console.log(JSON.stringify({ family, summary: result.summary, errorType: result.errorType, shape: result.shape, before: result.beforeSummary.diagnosticCodes, after: result.afterShape.diagnosticCodes }));
}
fs.writeFileSync(path.join(here, 'answers.json'), JSON.stringify(answers, null, 2));

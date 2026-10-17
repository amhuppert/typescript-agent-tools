import fs from 'node:fs';
import path from 'node:path';
import { typeOracle } from '../../tools/type-oracle.mjs';
import { lineStarts, position } from '../../tools/semantic-oracle.mjs';
const directory = path.resolve('fixtures/types/type-algebra');
const source = fs.readFileSync(path.join(directory, 'cases.ts'), 'utf8');
const markers = ['keyof', 'indexed', 'distributed', 'distribution', 'infer', 'modifiers', 'remap', 'template', 'uppercase', 'lowercase', 'capitalize', 'uncapitalize', 'awaited', 'builtin', 'noInfer', 'noInferRecovery', 'tuple', 'variadic', 'thisType', 'symbol', 'symbolKey', 'enumValue', 'freshObject', 'constant', 'factory', 'callback', 'genericClass', 'recursive', 'growing', 'any', 'unknown', 'missing', 'unknownConditional'];
const out = {};
for (const marker of markers) {
 const token = `/*#${marker}*/`;
 const index = source.indexOf(token);
 if (index < 0 || source.indexOf(token, index + 1) >= 0) throw Error('Nonunique marker ' + marker);
 const at = { file: 'cases.ts', ...position(lineStarts(source), index + token.length) };
 const request = { at, properties: ['distribution', 'modifiers', 'remap', 'thisType', 'symbolKey', 'freshObject', 'constant', 'factory', 'recursive', 'growing'].includes(marker) };
 const answer = await typeOracle(path.join(directory, 'tsconfig.json'), request);
 out[marker] = { request, answer };
 fs.writeFileSync('.cc/temp/types-algebra-observations.json', JSON.stringify(out, null, 2) + '\n');
 console.log(JSON.stringify({marker, summary: answer.summary, kind: answer.kind, error: answer.errorType, properties: answer.properties.map(p => [p.name, p.summary])}));
}

for (const [id, marker, config, extra] of [['builtinLoose', 'builtin', 'loose.json', {}], ['noInferRecoveryDiagnostics', 'noInferRecovery', 'tsconfig.json', {diagnostics: true}]]) {
 const token = `/*#${marker}*/`;
 const at = {file: 'cases.ts', ...position(lineStarts(source), source.indexOf(token) + token.length)};
 const request = {at, properties: false, ...extra};
 const answer = await typeOracle(path.join(directory, config), request);
 out[id] = {config, request, answer};
 fs.writeFileSync('.cc/temp/types-algebra-observations.json', JSON.stringify(out, null, 2) + '\n');
 console.log(JSON.stringify({id, summary: answer.summary, error: answer.errorType, diagnostics: answer.diagnostics.map(row => row.code)}));
}

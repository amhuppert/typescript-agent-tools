import fs from 'node:fs';
import path from 'node:path';
import { compilerHookOracle, typeOracle } from '../../tools/type-oracle.mjs';
import { lineStarts, position } from '../../tools/semantic-oracle.mjs';
const directory = path.resolve('fixtures/types/member-relations');
const source = fs.readFileSync(path.join(directory, 'cases.ts'), 'utf8');
const starts = lineStarts(source);
const markers = ['generic', 'stringUse', 'numberUse', 'partial', 'readonly', 'picked', 'omitted', 'remapped', 'ordinarySpread', 'overlapSpread', 'getOnly', 'getSpread', 'setOnly', 'setSpread', 'readonlySpread', 'host', 'annotated', 'satisfied', 'unrelated', 'annotatedUse', 'satisfiedUse', 'unrelatedUse', 'base', 'override', 'interface', 'implements', 'inlineLeft', 'inlineRight', 'inlineUse', 'interfaceLeft', 'interfaceRight', 'interfaceUnionUse', 'static', 'instance', 'staticUse', 'instanceUse', 'privateFirst', 'privateSecond', 'privateFirstUse', 'privateSecondUse', 'getter', 'setter', 'accessorUse'];
const out = {};
for (const marker of markers) {
  const token = `/*#${marker}*/`;
  const index = source.indexOf(token);
  if (index < 0 || source.indexOf(token, index + 1) >= 0) throw Error('Invalid marker ' + marker);
  const at = { file: 'cases.ts', ...position(starts, index + token.length) };
  const answer = await compilerHookOracle(path.join(directory, 'tsconfig.json'), { operation: 'member', at });
  out[marker] = answer;
  fs.writeFileSync('.cc/temp/types-member-relations-observations.json', JSON.stringify(out, null, 2) + '\n');
  const rows = origins => origins.map(origin => origin.location.start.line);
  console.log(JSON.stringify({marker, raw: rows(answer.rawOrigins), combined: rows(answer.combinedOrigins), roots: answer.roots.map(rows), searchRoots: answer.searchRoots.map(rows), groups: answer.groups.map(group => ({origins: rows(group.origins), occurrences: group.occurrences.map(row => row.start.line)})), failures: answer.demandFailures}));
}

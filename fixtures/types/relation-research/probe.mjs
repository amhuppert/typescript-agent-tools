import fs from 'node:fs';
import path from 'node:path';
import { compilerHookOracle } from './research-oracle.mjs';
import { lineStarts, position } from '../../../tools/semantic-oracle.mjs';

process.env.TYPE_SCRIPT_SEMANTIC_API ??= '/Users/alex/github/typescript-agent-tools/.cache/compiler-oracles/typescript-7.0.2/package';
process.env.TYPE_SCRIPT_NATIVE ??= '/Users/alex/github/typescript-agent-tools/.cache/compiler-oracles/typescript-darwin-x64-7.0.2/package/lib/tsc';

export function relationDepthSource(depth, leaf = 'same') {
  const wrap = (text) => '{ next: '.repeat(depth) + text + ' }'.repeat(depth);
  const targetLeaf = leaf === 'same' ? 'string' : 'number';
  return `type Source = ${wrap('{value: string}')};\ntype Target = ${wrap(`{value: ${targetLeaf}}`)};\ntype Query = Source extends Target ? true : false;\ndeclare const /*#query*/query: Query;\n`;
}
export function subtypeSource(length, strings) {
  const objects = Array.from({length: length-strings}, (_, index) => `{tag: ${index} as const}`);
  const primitives = Array.from({length: strings}, (_, index) => `"s${index}" as const`);
  return `const /*#query*/query = [${[...objects, ...primitives].join(',')}];\n`;
}
export function subtypeRemovalSource(length, removals) {
  const objects = Array.from({length: length-removals}, (_, index) => `declare const base${index}: {tag: ${index}};`);
  const redundant = Array.from({length: removals}, (_, index) => `declare const redundant${index}: {tag: 0; extra${index}: 0};`);
  const references = [...objects.map((_,index) => `base${index}`), ...redundant.map((_,index) => `redundant${index}`)];
  return `${[...objects,...redundant].join('\n')}\nconst /*#query*/query = [${references.join(',')}];\n`;
}
export function maybeSource(mode) {
  if (mode === 'active') return `interface Source {next: Source}\ninterface Target {next: Target}\ntype Query = Source extends Target ? true : false;\ndeclare const /*#query*/query: Query;\n`;
  if (mode === 'active-false') return `interface Source {next: Source; value: string}\ninterface Target {next: Target; value: number}\ntype Query = Source extends Target ? true : false;\ndeclare const /*#query*/query: Query;\n`;
  return `type Source<T> = {next: Source<Source<T>>};\ntype Target<T> = {next: Target<Target<T>>};\ntype Query = Source<string> extends Target<number> ? true : false;\ndeclare const /*#query*/query: Query;\n`;
}
export function allowanceSource(length, omit = -1) {
  const type = index => `{common:string; p${index}:string}`;
  const sources = Array.from({length},(_,index) => index).filter(index => index !== omit).map(type);
  const targets = Array.from({length},(_,index) => type(length-index-1));
  return `type Source = ${sources.join('|')};\ntype Target = ${targets.join('|')};\ntype Query = Source extends Target ? true : false;\ndeclare const /*#query*/query: Query;\n`;
}
export async function run(name, source) {
  const directory = path.resolve('.cc/temp/types-relation-limits', name);
  fs.mkdirSync(directory, {recursive:true});
  fs.writeFileSync(path.join(directory, 'cases.ts'), source);
  fs.writeFileSync(path.join(directory, 'tsconfig.json'), JSON.stringify({compilerOptions:{strict:true,target:'esnext',module:'esnext',noEmit:true}, files:['cases.ts']}));
  const offset = source.indexOf('/*#query*/') + '/*#query*/'.length;
  const answer = await compilerHookOracle(path.join(directory,'tsconfig.json'), {at:{file:'cases.ts',...position(lineStarts(source),offset)}});
  fs.writeFileSync(path.join(directory,'answer.json'), JSON.stringify(answer,null,2)+'\n');
  const observation = answer.beforeSummary;
  const events = observation.researchEvents;
  const compact = {summary: answer.summary?.length > 180 ? answer.summary.slice(0,180) : answer.summary, errorType:answer.errorType,
    counters:observation.counters, failures:observation.demandFailures, diagnosticCodes:answer.diagnosticCodes,
    arrayElement: answer.arrayElement,
    researchEventCount:events.length, events:events.filter(event => event.name !== 'relation_finished'),
    finished: events.filter(event => event.name === 'relation_finished' && event.cacheSize > 0).slice(-6)};
  console.log(JSON.stringify({name,...compact}));
  return answer;
}
if (process.argv[2] === 'depth') await run(`depth-${process.argv[3]}-${process.argv[4] ?? 'same'}`, relationDepthSource(Number(process.argv[3]),process.argv[4]));
if (process.argv[2] === 'subtype') await run(`subtype-${process.argv[3]}-${process.argv[4]}`, subtypeSource(Number(process.argv[3]),Number(process.argv[4])));
if (process.argv[2] === 'removal') await run(`removal-${process.argv[3]}-${process.argv[4]}`, subtypeRemovalSource(Number(process.argv[3]),Number(process.argv[4])));
if (process.argv[2] === 'maybe') await run(`maybe-${process.argv[3]}`, maybeSource(process.argv[3]));
if (process.argv[2] === 'allowance') await run(`allowance-${process.argv[3]}-${process.argv[4] ?? 'all'}`, allowanceSource(Number(process.argv[3]),Number(process.argv[4] ?? -1)));
if (process.argv[2] === 'initial') {
  for (const depth of [99,100,101]) await run(`depth-${depth}-same`,relationDepthSource(depth));
  for (const strings of [45,44,43]) await run(`subtype-1250-${strings}`,subtypeSource(1250,strings));
  for (const mode of ['active','deep']) await run(`maybe-${mode}`,maybeSource(mode));
  await run('allowance-100-all',allowanceSource(100));
}
if (process.argv[2] === 'refined') {
  for (const removals of [43,42,41]) await run(`removal-1250-${removals}`,subtypeRemovalSource(1250,removals));
  await run('allowance-100-reverse',allowanceSource(100));
}
if (process.argv[2] === 'final') {
  for (const depth of [99,100,101]) await run(`depth-${depth}-same`,relationDepthSource(depth));
  await run('depth-99-different',relationDepthSource(99,'different'));
  for (const removals of [43,42,41]) await run(`removal-1250-${removals}`,subtypeRemovalSource(1250,removals));
  for (const mode of ['active','active-false','deep']) await run(`maybe-${mode}`,maybeSource(mode));
  for (const length of [1998,1999,2000]) await run(`allowance-${length}-all`,allowanceSource(length));
}

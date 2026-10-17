import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { typeOracle, compilerHookOracle } from '../../../tools/type-oracle.mjs';
import { lineStarts, position } from '../../../tools/semantic-oracle.mjs';
const directory = fileURLToPath(new URL('./', import.meta.url));
const source = [
  'export declare let /*#property*/ property: { value?: string };',
  'export declare let /*#method*/ method: { value?(): number };',
  'export declare let /*#accessor*/ accessor: { get value(): string; set value(next: number); };',
  'export declare let /*#ignoredBadSetter*/ ignoredBadSetter: { get value(): string; set value(next: Missing); };',
].join('\n') + '\n';
fs.writeFileSync(path.join(directory, 'cases.ts'), source);
for (const [name, strict, exactOptionalPropertyTypes] of [['strict',true,false],['exact',true,true],['loose',false,false]])
  fs.writeFileSync(path.join(directory, `${name}.json`), JSON.stringify({ compilerOptions: {
    strict, exactOptionalPropertyTypes, noLib:true, target:'esnext', module:'esnext',
  }, files:['cases.ts'] },null,2)+'\n');
const root = process.cwd();
const inputs = ['tools/type-oracle.mjs','tools/semantic-oracle.mjs','tools/navigation-oracle.mjs',
  'fixtures/types/compiler-hook/checker-hook.go','fixtures/types/compiler-hook/ls-hook.go','fixtures/types/compiler-hook/oracle-test.go'];
const hashes = () => inputs.map(file => ({file,sha256:createHash('sha256').update(fs.readFileSync(path.join(root,file))).digest('hex')}));
const before = hashes();
const rows=[];
for (const config of ['strict.json','exact.json','loose.json']) {
  for (const match of source.matchAll(/\/\*#([^*]+)\*\/ /g)) {
    const at = {file:'cases.ts',...position(lineStarts(source),match.index+match[0].length)};
    const api = await typeOracle(path.join(directory,config),{at});
    const hook = await compilerHookOracle(path.join(directory,config),{at,operation:'type'});
    rows.push({config,marker:match[1],request:{at},api,hook});
    process.stdout.write(JSON.stringify({config,marker:match[1],summary:api.summary,errorType:api.errorType,
      property:api.properties[0], beforeSummary:hook.beforeSummary, afterSummaryDiagnostics:hook.diagnosticCodes})+'\n');
  }
}
const after=hashes();
fs.writeFileSync(path.join(directory,'answers.json'),JSON.stringify(rows,null,2)+'\n');
fs.writeFileSync(path.join(directory,'provenance.json'),JSON.stringify({scope:'throwaway fresh API/same-checker member display controls',before,after},null,2)+'\n');
if (JSON.stringify(before)!==JSON.stringify(after)) throw new Error('Member display tooling changed during capture');

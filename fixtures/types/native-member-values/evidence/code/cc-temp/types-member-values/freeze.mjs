import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
const root=process.cwd();
const here=path.join(root,'.cc/temp/types-member-values');
const out=path.join(here,'evidence');
if(fs.existsSync(out)) throw new Error('Refuse to replace frozen member evidence');
const hash=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const rows=[...JSON.parse(fs.readFileSync(path.join(here,'answers.json'),'utf8')),
  ...JSON.parse(fs.readFileSync(path.join(here,'extra-answers.json'),'utf8'))];
assert.equal(rows.length,24);
const captured=new Map();
for(const row of rows){
  assert.equal(row.answer.status,'observed');
  assert.equal(row.answer.compiler,'7.0.2');
  assert.deepEqual(row.answer.inputFiles,row.answer.inputHashes.map(item=>item.file));
  for(const input of row.answer.inputHashes){
    if(captured.has(input.file)) assert.equal(captured.get(input.file).sha256,input.sha256);
    const bytes=fs.readFileSync(input.file);
    assert.equal(hash(bytes),input.sha256,input.file);
    const relative=path.relative(root,input.file);
    assert.ok(relative&&!relative.startsWith('..')&&!path.isAbsolute(relative));
    captured.set(input.file,{file:input.file,sha256:input.sha256,
      archive:path.join('captured-inputs',relative),bytes});
  }
}
fs.mkdirSync(out,{recursive:true});
const write=(relative,bytes)=>{const target=path.join(out,relative);
  fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,bytes);};
for(const input of captured.values()) write(input.archive,input.bytes);
const codeFiles=[
  ...['setup.mjs','probe.mjs','probe-extra.mjs','freeze.mjs','oracle.mjs','checker-hook.go','ls-hook.go','oracle-test.go']
    .map(name=>path.join(here,name)),
  ...['semantic-oracle.mjs','vendor-typescript-libs.mjs','navigation-oracle.mjs']
    .map(name=>path.join(root,'tools',name)),
];
const code=codeFiles.map(file=>{const bytes=fs.readFileSync(file);
  const archive=path.join('code',path.relative(root,file));write(archive,bytes);
  return{file,archive,sha256:hash(bytes)};});
const answersBytes=Buffer.from(JSON.stringify(rows,null,2)+'\n');
write('answers.json',answersBytes);
const provenance={schema:1,compiler:'7.0.2',
  sourceCommit:'2bd066d87f5bafd315be9f40889d0a60b9e58e0b',
  rows:24,history:'baseline',answerSha256:hash(answersBytes),
  capturedInputs:[...captured.values()].map(({bytes,...input})=>input),code,
  command:{setup:'node .cc/temp/types-member-values/setup.mjs',
    probes:['node .cc/temp/types-member-values/probe.mjs','node .cc/temp/types-member-values/probe-extra.mjs'],
    environment:{TYPE_SCRIPT_SEMANTIC_API:'/Users/alex/github/typescript-agent-tools/.cache/compiler-oracles/typescript-7.0.2/package',
      TYPE_SCRIPT_NATIVE:'/Users/alex/github/typescript-agent-tools/.cache/compiler-oracles/typescript-darwin-x64-7.0.2/package/lib/tsc'}},
  limitations:[
    'Disposable native checker shim research, not a registered product validation run.',
    'Generated copied oracle imports the exact original absolute dependency paths. Archived hashes bind the read sources; this archive is not a standalone replay bundle.',
    'Raw read and its TypeToString precede nonmissing read, then write, then own-location reads. Signatures may consume dependencies during the summary.',
    'Diagnostic snapshots read only accumulated checker diagnostics; no semantic diagnostic checking prelude. Initial noLib TS2318 diagnostics are retained separately.',
    'Only exactOptionalPropertyTypes plus strictNullChecks false TS5052 is permitted by this disposable replay. Native API and replay Program diagnostics retain 5052; all other setup diagnostics still reject.',
    'Single-file early ordinary member family, no full method body/context, alias, inheritance, circular accessor or property initialization proof.',
  ]};
write('provenance.json',Buffer.from(JSON.stringify(provenance,null,2)+'\n'));
console.log(JSON.stringify({rows:rows.length,inputs:captured.size,codeFiles:code.length,answerSha256:provenance.answerSha256,out}));

import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const root = path.resolve('.cc/temp/types-relation-limits');
const binary = '/Users/alex/github/typescript-agent-tools/.cache/compiler-oracles/typescript-darwin-x64-7.0.2/package/lib/tsc';
const names = ['depth-99-same','depth-100-same','depth-101-same','depth-99-different','removal-1250-43','removal-1250-42','removal-1250-41','maybe-active','maybe-active-false','maybe-deep','allowance-1998-all','allowance-1999-all','allowance-2000-all'];
const version = spawnSync(binary,['--version'],{encoding:'utf8'});
if(version.status !== 0 || version.stdout.trim() !== 'Version 7.0.2') throw new Error('Native version pin failed');
const rows = [];
for(const name of names){
  const directory = path.join(root,name);
  if(!fs.existsSync(path.join(directory,'answer.json'))) continue;
  const result = spawnSync(binary,['-p',path.join(directory,'tsconfig.json')],{encoding:'utf8',timeout:180000,maxBuffer:1048576});
  if(result.error || result.signal) throw result.error ?? new Error(result.signal);
  const output = result.stdout+result.stderr;
  const row = {name,sourceSha256:createHash('sha256').update(fs.readFileSync(path.join(directory,'cases.ts'))).digest('hex'),status:result.status,codes:[...new Set([...output.matchAll(/error TS(\d+)/g)].map(match => Number(match[1])))],stdout:result.stdout,stderr:result.stderr};
  rows.push(row);console.log(JSON.stringify({name,status:row.status,codes:row.codes}));
}
fs.writeFileSync(path.join(root,'native-controls.json'),JSON.stringify({version:version.stdout.trim(),binary,rows},null,2)+'\n');

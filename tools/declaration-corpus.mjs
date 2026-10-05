import { runParser, parserFailure } from './parser-process.mjs';
import fs from 'node:fs';
import path from 'node:path';
const directory = process.argv[2];
if (!directory) throw new Error('Usage: node tools/declaration-corpus.mjs PATH_TO_TYPESCRIPT_LIB');
const binary = process.env.TSPARSE ?? path.resolve('target/debug/tsparse');
let total = 0, bytes = 0;
const failures = [];
for (const name of fs.readdirSync(directory).filter(name => name.endsWith('.d.ts'))) {
  const file = path.join(directory, name);
  const result = runParser(binary, ['--check', file]);
  total++; bytes += fs.statSync(file).size;
  if (result.outcome !== 'accepted') failures.push({ name, ...parserFailure(result) });
}
console.log(JSON.stringify({ total, bytes, failures }, null, 2));
process.exitCode = failures.length ? 1 : 0;

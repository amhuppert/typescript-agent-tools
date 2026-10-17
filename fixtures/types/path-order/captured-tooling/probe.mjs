import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
const source = '/Users/alex/github/typescript-agent-tools/.cache/typescript-go-7.0.2';
const pin = '2bd066d87f5bafd315be9f40889d0a60b9e58e0b';
const command = (name, args, options = {}) => {
  const result = spawnSync(name, args, { cwd: source, encoding: 'utf8', ...options });
  if (result.status !== 0) throw new Error(`${name} failed: ${result.stdout}${result.stderr}`);
  return result.stdout.trim();
};
if (command('git', ['rev-parse', 'HEAD']) !== pin || command('git', ['status', '--porcelain', '--untracked-files=all']))
  throw new Error('Exact clean native source pin required');
const here = path.resolve('.cc/temp/types-path-order');
const file = path.join(here, 'observe_test.go');
const overlay = path.join(here, 'overlay.json');
const output = path.join(here, 'answer.json');
fs.writeFileSync(overlay, JSON.stringify({ Replace: { [path.join(source, 'internal/stringutil/zz_agent_path_order_test.go')]: file } }));
command('go', ['test', '-p', '2', '-overlay', overlay, './internal/stringutil', '-run', '^TestAgentPathOrder$', '-count=1'],
  { env: { ...process.env, GOMAXPROCS: '2', AGENT_PATH_ORDER_OUTPUT: output } });
const answer = JSON.parse(fs.readFileSync(output, 'utf8'));
answer.sourceCommit = pin;
answer.comparerHash = createHash('sha256').update(fs.readFileSync(path.join(source, 'internal/stringutil/compare.go'))).digest('hex');
fs.writeFileSync(output, JSON.stringify(answer, null, 2) + '\n');
console.log(JSON.stringify(answer));

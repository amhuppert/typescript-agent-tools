import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {test} from 'node:test';
import {loadSemanticApi, normalizeOracle, SemanticSession} from './semantic-oracle.mjs';

// Q1 deliberately excludes properties of unresolved value receivers, while
// retaining the import failure itself. Keep this normalization pinned to the
// native symbol query independently of the checker’s dependency provenance.
test('native default-import failure retains the lexical receiver distinction', async t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-default-imports-'));
  t.after(() => fs.rmSync(root, {recursive: true, force: true}));
  const semantic = await loadSemanticApi();
  const session = new SemanticSession(semantic);
  t.after(() => session.close());
  const files = {
    'tsconfig.json': JSON.stringify({compilerOptions: {noLib: true, strict: true},
      files: ['app.ts', 'ambient.d.ts']}),
    'plain.d.ts': 'export declare const x: number;\n',
    'marked.d.ts': 'export declare const __esModule: true; export declare const x: number;\n',
    'failed.d.ts': 'export { missing as default };\n',
    'ordinary.ts': 'export const x = 1;\n',
    'ambient.d.ts': 'declare module "plain" { export const x: number; }\ndeclare module "marked" { export const __esModule: true; export const x: number; }\n',
    'app.ts': [
      'import P from "./plain";',
      'import M from "./marked";',
      'import F from "./failed";',
      'import O from "./ordinary";',
      'import AP from "plain";',
      'import AM from "marked";',
      'import * as NS from "./plain";',
      'P.x;', 'M.x;', 'F.x;', 'O.x;', 'AP.x;', 'AM.x;',
      'M.x.more;', 'NS.missing;', 'type Qualified = M.x;',
    ].join('\n'),
  };
  for (const [name, source] of Object.entries(files)) fs.writeFileSync(path.join(root, name), source);
  const oracle = normalizeOracle(session.collect(path.join(root, 'tsconfig.json')));
  assert.equal(oracle.compiler, '7.0.2');
  const properties = oracle.tokens.filter(token => token.location.file === 'app.ts' && token.syntax === 'property');
  assert.deepEqual(properties.map(token => [token.location.start.line, token.name, token.outcome]), [
    [8, 'x', 'groups'], [9, 'x', 'unsupported'], [10, 'x', 'unsupported'],
    [11, 'x', 'unsupported'], [12, 'x', 'groups'], [13, 'x', 'unsupported'],
    [14, 'x', 'unsupported'], [14, 'more', 'unsupported'],
    [15, 'missing', 'unresolved'],
  ]);
  const qualified = oracle.tokens.find(token => token.location.file === 'app.ts'
    && token.location.start.line === 16 && token.name === 'x');
  assert.equal(qualified.outcome, 'unresolved');
});

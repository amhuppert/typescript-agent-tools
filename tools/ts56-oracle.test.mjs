import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { QueryError, ts56Oracle } from './ts56-oracle.mjs';
import { loadTypeScript56, locateTypeScript56, typeScript56Unavailable, TS56_VARIABLE } from './ts56-compiler.mjs';

// Real-compiler tests are skipped, with this reason, when the pinned
// TypeScript 5.6.3 package is not found.
const compilerUnavailable = typeScript56Unavailable();
const oracle = fileURLToPath(new URL('./ts56-oracle.mjs', import.meta.url));
const TOOLS = fileURLToPath(new URL('.', import.meta.url));

const FILES = {
  'tsconfig.json': JSON.stringify({ compilerOptions: {
    noLib: true, types: [], strict: true, module: 'esnext', moduleResolution: 'bundler', esModuleInterop: true, incremental: true,
  } }),
  'a.ts': 'export const value = 1;\n',
  'b.ts': 'import { value } from "./a";\nexport const doubled = value + value;\n',
};

function project(t, files = FILES) {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-ts56-')));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const [file, text] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), text);
  }
  return { root, config: path.join(root, 'tsconfig.json') };
}

function run(config, words, env = {}) {
  const result = spawnSync(process.execPath, [oracle, config, ...words],
    { cwd: path.dirname(config), env: { ...process.env, ...env }, encoding: 'utf8', timeout: 60_000 });
  return { status: result.status, stdout: result.stdout, stderr: result.stderr };
}

const buildInfo = root => fs.readdirSync(root, { recursive: true }).map(String).filter(file => file.endsWith('.tsbuildinfo'));
const span = (line, column, endColumn) => ({ start: { line, column }, end: { line, column: endColumn } });
const reference = (file, line, column, definition, write) => ({ file, line, column, definition, write });

test('locating: the variable names the 5.6.3 package directory, else the canonical checkout holds it', t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-ts56-package-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const absent = `${TS56_VARIABLE} names ${directory}, which holds no package.json`;
  assert.equal(typeScript56Unavailable(directory), absent);
  assert.throws(() => locateTypeScript56(directory), { message: absent });
  // Another version is present, so its tests run, and it cannot be used.
  fs.writeFileSync(path.join(directory, 'package.json'), '{ "name": "typescript", "version": "5.6.2" }');
  assert.equal(typeScript56Unavailable(directory), false);
  assert.throws(() => locateTypeScript56(directory), { message: `expected TypeScript 5.6.3 at ${directory}, found version 5.6.2` });
  fs.writeFileSync(path.join(directory, 'package.json'), '{ "name": "typescript", "version": "5.6.3" }');
  assert.deepEqual(locateTypeScript56(directory), { packageDir: directory });

  // Unset, the canonical checkout's cache, found from this worktree.
  const common = spawnSync('git', ['rev-parse', '--path-format=absolute', '--git-common-dir'], { cwd: TOOLS, encoding: 'utf8' });
  assert.equal(common.status, 0, common.stderr);
  const cached = path.join(path.dirname(common.stdout.trim()), '.cache/compiler-oracles/typescript-5.6.3/package');
  const unset = typeScript56Unavailable('');
  if (unset) assert.equal(unset, `${TS56_VARIABLE} is unset and no TypeScript 5.6.3 package is at ${cached}`);
  else assert.deepEqual(locateTypeScript56(''), { packageDir: cached });
});

test('locating: a missing or other-version compiler cannot run the oracle (exit 2, stated reason)', t => {
  const { root, config } = project(t);
  const missing = path.join(root, 'no-compiler');
  assert.deepEqual(run(config, ['type', 'a.ts:1:14'], { [TS56_VARIABLE]: missing }),
    { status: 2, stdout: '', stderr: `ts56-oracle: ${TS56_VARIABLE} names ${missing}, which holds no package.json\n` });
  const other = path.join(root, 'other');
  fs.mkdirSync(other);
  fs.writeFileSync(path.join(other, 'package.json'), '{ "name": "typescript", "version": "5.7.2" }');
  assert.deepEqual(run(config, ['type', 'a.ts:1:14'], { [TS56_VARIABLE]: other }),
    { status: 2, stdout: '', stderr: `ts56-oracle: expected TypeScript 5.6.3 at ${other}, found version 5.7.2\n` });
});

test('real compiler: quick info and references of an exported constant and its import', { skip: compilerUnavailable }, t => {
  const { root, config } = project(t);
  const result = run(config, [
    'type', 'a.ts:1:14', 'type', 'b.ts:1:10', 'type', 'b.ts:2:24', 'type', 'b.ts:2:22',
    'references', 'a.ts:1:14', 'references', 'b.ts:2:24', 'references', 'b.ts:2:22',
  ]);
  assert.equal(result.status, 0, result.stderr);
  const alias = '(alias) const value: 1\nimport value';
  assert.deepEqual(JSON.parse(result.stdout), {
    config,
    version: '5.6.3',
    results: [
      { query: 'type a.ts:1:14', quickInfo: { text: 'const value: 1', span: span(1, 14, 19) } },
      { query: 'type b.ts:1:10', quickInfo: { text: alias, span: span(1, 10, 15) } },
      { query: 'type b.ts:2:24', quickInfo: { text: alias, span: span(2, 24, 29) } },
      // The `=` of `doubled = ...`.
      { query: 'type b.ts:2:22', quickInfo: null },
      // From the declaration: its own symbol's references, then the import's.
      { query: 'references a.ts:1:14', references: [
        reference('a.ts', 1, 14, true, true), reference('b.ts', 1, 10, false, true),
        reference('b.ts', 2, 24, false, false), reference('b.ts', 2, 32, false, false),
      ] },
      // From a use, which is not a declaration name, TypeScript marks no definition.
      { query: 'references b.ts:2:24', references: [
        reference('b.ts', 1, 10, false, true), reference('b.ts', 2, 24, false, false),
        reference('b.ts', 2, 32, false, false), reference('a.ts', 1, 14, false, true),
      ] },
      { query: 'references b.ts:2:22', references: null },
    ],
  });
  // `incremental: true` is forced off: nothing is written into the project.
  assert.deepEqual(buildInfo(root), []);
});

test('real compiler: lines break at LF, CR, CRLF, U+2028, and U+2029, and columns count UTF-16 code units', { skip: compilerUnavailable }, t => {
  const { config } = project(t, {
    ...FILES,
    'lines.ts': ['export const v1 = 1;', '\n', 'export const v2 = 2;', '\r', 'export const v3 = 3;', '\r\n',
      'export const v4 = 4;', ' ', 'export const v5 = 5;', ' ', 'export const v6 = 6;', '\n'].join(''),
    'wide.ts': 'const s = "\u{1F600}"; export const after = s;\n',
  });
  const compiler = loadTypeScript56();
  const answer = query => ts56Oracle(config, [query], compiler).results[0];
  for (let line = 1; line <= 6; line++) {
    assert.deepEqual(answer(`type lines.ts:${line}:14`).quickInfo, { text: `const v${line}: ${line}`, span: span(line, 14, 16) });
    // Each line holds 20 code units: column 21 is its end, and 22 is past it.
    assert.deepEqual(answer(`type lines.ts:${line}:21`), { query: `type lines.ts:${line}:21`, quickInfo: null });
    assert.throws(() => answer(`type lines.ts:${line}:22`),
      error => error instanceof QueryError && error.message === `type lines.ts:${line}:22: line ${line} of lines.ts has 20 UTF-16 code units`);
  }
  // The emoji is two UTF-16 code units: `after` starts at column 30, the 29th code point.
  assert.deepEqual(answer('type wide.ts:1:30').quickInfo, { text: 'const after: "\u{1F600}"', span: span(1, 30, 35) });
  assert.deepEqual(answer('type wide.ts:1:29').quickInfo, null);
  assert.deepEqual(answer('references wide.ts:1:7').references,
    [reference('wide.ts', 1, 7, true, true), reference('wide.ts', 1, 38, false, false)]);
});

test('real compiler: a bad query exits 1 before any answer, and wrong usage exits 2', { skip: compilerUnavailable }, t => {
  const { root, config } = project(t);
  const bad = [
    [['size', 'a.ts:1:14'], 'size a.ts:1:14: the query kind must be type, references, or specifier'],
    [['specifier', 'b.ts'], 'specifier b.ts: expected specifier IMPORTER=TARGET'],
    [['specifier', '=a.ts'], 'specifier =a.ts: expected specifier IMPORTER=TARGET'],
    [['specifier', 'b.ts=c.ts'], 'specifier b.ts=c.ts: c.ts is not a file of the program'],
    [['type', 'a.ts:1'], 'type a.ts:1: expected type FILE:LINE:COLUMN with a one-based LINE and COLUMN'],
    [['type', 'a.ts:0:1'], 'type a.ts:0:1: expected type FILE:LINE:COLUMN with a one-based LINE and COLUMN'],
    [['references', 'c.ts:1:1'], 'references c.ts:1:1: c.ts is not a file of the program'],
    [['type', 'a.ts:3:1'], 'type a.ts:3:1: a.ts has 2 lines'],
    [['type', 'a.ts:1:25'], 'type a.ts:1:25: line 1 of a.ts has 23 UTF-16 code units'],
  ];
  for (const [words, reason] of bad) {
    // A valid query first: no document is printed for it either.
    assert.deepEqual(run(config, ['type', 'a.ts:1:14', ...words]), { status: 1, stdout: '', stderr: `ts56-oracle: ${reason}\n` });
  }
  assert.throws(() => ts56Oracle(config, ['type a.ts:3:1']), QueryError);
  // An absolute FILE names the same file.
  assert.equal(JSON.parse(run(config, ['type', path.join(root, 'a.ts:1:14')]).stdout).results[0].quickInfo.text, 'const value: 1');
  for (const words of [[], ['type'], ['type', 'a.ts:1:14', 'references']]) assert.equal(run(config, words).status, 2);
  assert.equal(run(path.join(root, 'missing.json'), ['type', 'a.ts:1:14']).status, 2);
});

test('real compiler: module specifiers follow baseUrl, the project boundary, and package scopes', { skip: compilerUnavailable }, t => {
  const options = { noLib: true, types: [], strict: true, target: 'es2022', module: 'esnext', moduleResolution: 'bundler', esModuleInterop: true };
  const { root } = project(t, {
    'app/tsconfig.json': JSON.stringify({ compilerOptions: { ...options, baseUrl: '..' }, include: ['src', '../shared', 'scoped'] }),
    'shared/lib.ts': 'export const lib = 1;\n',
    'app/src/x.ts': 'export const x = 1;\n',
    'app/src/deep/use.ts': 'import { x } from "../x";\nexport const use = x;\n',
    'app/scoped/package.json': '{"name":"scoped"}',
    'app/scoped/thing.ts': 'export const thing = 1;\n',
  });
  const result = run(path.join(root, 'app/tsconfig.json'), [
    'specifier', 'src/deep/use.ts=../shared/lib.ts', 'specifier', 'src/deep/use.ts=scoped/thing.ts',
    'specifier', 'src/deep/use.ts=src/x.ts',
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout).results, [
    // Outside the project, the path from `baseUrl`.
    { query: 'specifier src/deep/use.ts=../shared/lib.ts', specifiers: ['shared/lib'] },
    // In another package scope, the path from `baseUrl`.
    { query: 'specifier src/deep/use.ts=scoped/thing.ts', specifiers: ['app/scoped/thing'] },
    // A written import is reused.
    { query: 'specifier src/deep/use.ts=src/x.ts', specifiers: ['../x'] },
  ]);
});

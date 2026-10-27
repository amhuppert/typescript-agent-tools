import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { typeOracle, compilerHookOracle } from './type-oracle.mjs';
import { lineStarts, position } from './semantic-oracle.mjs';
import { scratchPath } from './harness-scratch.mjs';

function project(t, files, selectedFile = 'cases.ts', configFile = 'tsconfig.json') {
  const root = fs.mkdtempSync(scratchPath('type-oracle-review-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const [file, text] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), text);
  }
  const source = files[selectedFile];
  const marker = '/*#query*/';
  const offset = source.indexOf(marker);
  assert.notEqual(offset, -1);
  const anchorFile = path.relative(path.dirname(configFile), selectedFile).split(path.sep).join('/');
  return { config: path.join(root, configFile), request: { at: { file: anchorFile,
    ...position(lineStarts(source), offset + marker.length) } } };
}

test('compiler hook preserves inherited configuration accepted by the pinned native program', async t => {
  const { config, request } = project(t, {
    'tsconfig.json': '{"extends":"./base.json","files":["cases.ts"]}',
    'base.json': '{"compilerOptions":{"noLib":true,"strict":false,"noEmit":true}}',
    'cases.ts': 'declare const object: { value?: string };\nexport const /*#query*/value = object.value;\n',
  });
  const native = await typeOracle(config, request);
  assert.equal(native.compilerOptions.strict, false);
  assert.equal(native.summary, 'string');
  const observed = await compilerHookOracle(config, request).then(
    result => ({ result }), error => ({ error: error.message }));
  assert.equal(observed.error, undefined, 'the hook must load the same accepted inherited configuration');
  assert.equal(observed.result.summary, native.summary);
});

test('compiler hook preserves package exports metadata that determines a demand type', async t => {
  const { config, request } = project(t, {
    'tsconfig.json': '{"compilerOptions":{"noLib":true,"strict":true,"module":"nodenext","noEmit":true},"files":["cases.mts"]}',
    'cases.mts': 'import { dep } from "dual";\nexport const /*#query*/value = dep;\n',
    'node_modules/dual/package.json': '{"name":"dual","version":"1.0.0","exports":{".":{"import":"./index.d.mts","require":"./index.d.cts"}}}',
    'node_modules/dual/index.d.mts': 'export declare const dep: "import";\n',
    'node_modules/dual/index.d.cts': 'export declare const dep: "require";\n',
  }, 'cases.mts');
  const native = await typeOracle(config, request);
  assert.equal(native.summary, '"import"');
  const observed = await compilerHookOracle(config, request);
  assert.equal(observed.summary, native.summary, 'same source program and package exports must yield the same demand');
});

test('compiler hook preserves the shorthand value-symbol selection question', async t => {
  const { config, request } = project(t, {
    'tsconfig.json': '{"compilerOptions":{"noLib":true,"strict":true,"noEmit":true},"files":["cases.ts"]}',
    'cases.ts': 'const value = "fixed";\nexport const object = { /*#query*/value };\n',
  });
  const native = await typeOracle(config, request);
  assert.equal(native.kind, 'value');
  assert.equal(native.summary, '"fixed"');
  const observed = await compilerHookOracle(config, request);
  assert.equal(observed.summary, native.summary,
    'shorthand selects the value symbol, whose type differs from the object property');
});

test('compiler source pin rejects untracked Go build inputs', async t => {
  const common = spawnSync('git', ['rev-parse', '--path-format=absolute', '--git-common-dir'], { encoding: 'utf8' });
  assert.equal(common.status, 0, common.stderr);
  const source = process.env.TYPE_SCRIPT_GO_SOURCE || path.join(path.dirname(common.stdout.trim()), '.cache/typescript-go-7.0.2');
  const root = fs.mkdtempSync(scratchPath('type-oracle-source-review-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const clone = path.join(root, 'source');
  const copied = spawnSync('git', ['clone', '--quiet', '--shared', source, clone], { encoding: 'utf8' });
  assert.equal(copied.status, 0, copied.stderr);
  fs.writeFileSync(path.join(clone, 'internal/checker/zz_untracked_review.go'), 'package checker\nfunc AgentOracleReviewUnexpectedSource() {}\n');
  const oldSource = process.env.TYPE_SCRIPT_GO_SOURCE;
  const oldPath = process.env.PATH;
  const oldMarker = process.env.AGENT_ORACLE_REVIEW_MARKER;
  const shimDirectory = path.join(root, 'bin');
  const marker = path.join(root, 'compiler-executed');
  fs.mkdirSync(shimDirectory);
  fs.writeFileSync(path.join(shimDirectory, 'go'), '#!/usr/bin/env node\nrequire("node:fs").writeFileSync(process.env.AGENT_ORACLE_REVIEW_MARKER,"reached");\nprocess.stderr.write("untracked source reached compiler execution\\n");\nprocess.exit(7);\n', { mode: 0o755 });
  process.env.TYPE_SCRIPT_GO_SOURCE = clone;
  process.env.PATH = `${shimDirectory}${path.delimiter}${oldPath}`;
  process.env.AGENT_ORACLE_REVIEW_MARKER = marker;
  try {
    const fixtures = fileURLToPath(new URL('../fixtures/types/selection/', import.meta.url));
    const text = fs.readFileSync(path.join(fixtures, 'cases.ts'), 'utf8');
    const offset = text.indexOf('/*#class*/') + '/*#class*/'.length;
    const observed = await compilerHookOracle(path.join(fixtures, 'tsconfig.json'), { at: { file: 'cases.ts',
      ...position(lineStarts(text), offset) } }).then(result => ({ result }), error => ({ error: error.message }));
    assert.equal(fs.existsSync(marker), false, 'an untracked build input must fail source validation before invoking Go');
    assert.match(observed.error, /source|pinned|dirty/i);
  } finally {
    if (oldSource === undefined) delete process.env.TYPE_SCRIPT_GO_SOURCE;
    else process.env.TYPE_SCRIPT_GO_SOURCE = oldSource;
    if (oldPath === undefined) delete process.env.PATH;
    else process.env.PATH = oldPath;
    if (oldMarker === undefined) delete process.env.AGENT_ORACLE_REVIEW_MARKER;
    else process.env.AGENT_ORACLE_REVIEW_MARKER = oldMarker;
  }
});

test('cold compiler-hook observations do not claim the baseline candidate schedule', async () => {
  const fixtures = fileURLToPath(new URL('../fixtures/types/late-members/', import.meta.url));
  const text = fs.readFileSync(path.join(fixtures, 'cases.ts'), 'utf8');
  const offset = text.indexOf('/*#named*/') + '/*#named*/'.length;
  const observed = await compilerHookOracle(path.join(fixtures, 'tsconfig.json'), { operation: 'member', history: 'cold',
    at: { file: 'cases.ts', ...position(lineStarts(text), offset) } });
  assert.deepEqual(observed.prelude, []);
  assert.notEqual(observed.schedule?.relationshipBeforeCandidates, true,
    'cold mode skips the after-search-set candidate schedule and must describe that fact');
});

test('compiler hook checks candidates in normalized project-relative path order', async t => {
  const { config, request } = project(t, {
    'project/tsconfig.json': '{"compilerOptions":{"noLib":true,"strict":true,"noEmit":true},"files":["inside.ts","../zzz.ts"]}',
    'project/inside.ts': 'interface Receiver { /*#query*/m(): void }\ndeclare const inside: Receiver; inside.m();\n',
    'zzz.ts': 'declare const outside: Receiver; outside.m();\n',
  }, 'project/inside.ts', 'project/tsconfig.json');
  const observed = await compilerHookOracle(config, { ...request, operation: 'member' });
  assert.deepEqual(observed.prelude.map(call => call.location.file), ['../zzz.ts', 'inside.ts'],
    'absolute file-name sorting must not replace the specified normalized relative order');
});

test('compiler hook fixture paths preserve spaces, percent signs and non-ASCII checkout names', async t => {
  const root = fs.mkdtempSync(scratchPath('type-oracle-path-review-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const relocated = path.join(root, 'oracle fixtures é %');
  const toolsDirectory = path.join(relocated, 'tools');
  const hooksDirectory = path.join(relocated, 'fixtures/types/compiler-hook');
  fs.mkdirSync(toolsDirectory, { recursive: true });
  fs.mkdirSync(hooksDirectory, { recursive: true });
  for (const file of ['type-oracle.mjs', 'type-oracle-filesystem.mjs', 'type-oracle-compiler.mjs',
    'semantic-oracle.mjs', 'navigation-oracle.mjs', 'corpus-selection.mjs', 'vendor-typescript-libs.mjs',
    'harness-scratch.mjs'])
    fs.copyFileSync(fileURLToPath(new URL(file, import.meta.url)), path.join(toolsDirectory, file));
  for (const file of ['checker-hook.go', 'ls-hook.go', 'oracle-test.go'])
    fs.copyFileSync(fileURLToPath(new URL(`../fixtures/types/compiler-hook/${file}`, import.meta.url)), path.join(hooksDirectory, file));
  const source = 'export const value = 42;\n';
  const config = path.join(relocated, 'tsconfig.json');
  fs.writeFileSync(config, '{"compilerOptions":{"noLib":true,"strict":true,"noEmit":true},"files":["cases.ts"]}');
  fs.writeFileSync(path.join(relocated, 'cases.ts'), source);
  const request = { at: { file: 'cases.ts', ...position(lineStarts(source), source.indexOf('value')) } };
  const oracle = await import(pathToFileURL(path.join(toolsDirectory, 'type-oracle.mjs')).href);
  assert.equal((await oracle.typeOracle(config, request)).summary, '42');
  const observed = await oracle.compilerHookOracle(config, request).then(
    result => ({ result }), error => ({ error: error.message }));
  assert.equal(observed.error, undefined,
    'a valid relocated checkout must open its actual fixture filenames, without URL percent encoding');
  assert.equal(observed.result.summary, '42');
});

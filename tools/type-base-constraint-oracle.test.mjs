// Fresh native questions pin raw declared constraints independently of bases.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadSemanticApi } from './semantic-oracle.mjs';

const directory = fileURLToPath(new URL('../fixtures/types/base-constraints/', import.meta.url));
const source = fs.readFileSync(path.join(directory, 'cases.ts'), 'utf8');

async function observe(marker, question, configName = 'tsconfig.json', variant) {
  const working = variant ? path.join(directory, 'limits', variant) : directory;
  const native = await loadSemanticApi();
  assert.equal(native.version, '7.0.2');
  const api = new native.API({ tsserverPath: native.binary, cwd: working });
  let snapshot;
  try {
    const config = path.join(working, configName);
    snapshot = api.updateSnapshot({ openProject: config });
    const project = snapshot.getProject(config);
    assert.ok(project, 'pinned project opens');
    const program = project.program;
    for (const rows of [program.getConfigFileParsingDiagnostics(), program.getProgramDiagnostics(),
      program.getSyntacticDiagnostics(), program.getBindDiagnostics()]) assert.deepEqual(rows, []);
    const parsed = program.getSourceFile(path.join(working, 'cases.ts'));
    const text = variant ? parsed.text : source;
    const offset = text.indexOf(`/*#${marker}*/`) + marker.length + 5;
    assert.ok(offset > marker.length + 4, marker);
    const token = native.getTokenAtPosition(parsed, offset);
    // Identifier/this token acquisition can return a synthetic token. The
    // complete native TypeReference/ThisType node owns this type question.
    const root = project.checker.getTypeAtLocation(token.parent);
    if (!variant?.startsWith('repeat-')) assert.equal(root.isTypeParameter(), true, marker);
    assert.equal(root.isErrorType(), false, marker);
    const child = question === 'base' ? project.checker.getBaseConstraintOfType(root)
      : project.checker.getConstraintOfTypeParameter(root);
    return { root: project.checker.typeToString(root), isThis: root.isThisType === true,
      child: child ? { summary: project.checker.typeToString(child), flags: child.flags,
        errorType: child.isErrorType(), same: child.id === root.id } : null };
  } finally { try { snapshot?.dispose(); } finally { api.close(); } }
}

test('native parameter bases flatten chains while declared constraints retain symbolic parameter identities', async () => {
  const expected = [
    ['literal', 'U', '"ready"'], ['free', 'U', null], ['either', 'U | V', '"left" | 7'],
    ['partial', 'U | V', null], ['object', 'U', '{ value: string; }'],
    ['reference', 'Box<number>', 'Box<number>'], ['recursive', '{ next: T; }', '{ next: T; }'],
  ];
  for (const config of ['tsconfig.json', 'loose.json']) for (const [marker, declared, base] of expected) {
    const raw = await observe(marker, 'constraint', config);
    const resolved = await observe(marker, 'base', config);
    assert.equal(raw.child?.summary, declared, `${config}: ${marker} constraint`);
    assert.equal(raw.child?.errorType, false, marker);
    assert.equal(resolved.child?.summary ?? null, base, `${config}: ${marker} base`);
    if (base !== null) assert.equal(resolved.child.errorType, false, marker);
  }
});

test('native circular constraints are absent while unresolved annotations retain actual error types', async () => {
  for (const marker of ['direct', 'mutual']) for (const question of ['constraint', 'base']) {
    assert.equal((await observe(marker, question)).child, null, `${marker}: ${question}`);
  }
  for (const question of ['constraint', 'base']) {
    const missing = await observe('missing', question);
    assert.equal(missing.child.summary, 'MissingType');
    assert.equal(missing.child.errorType, true);
  }
});

test('native synthetic this has its actual target as base without recursively demanding target members', async () => {
  for (const question of ['constraint', 'base']) {
    const answer = await observe('this', question);
    assert.equal(answer.root, 'this');
    assert.equal(answer.isThis, true);
    assert.equal(answer.child.summary, 'ThisRoot<T>');
    assert.equal(answer.child.errorType, false);
  }
});

test('native normal base approximation remains distinct at recursion identity and depth boundaries', async () => {
  for (const count of [9,10,11,50,51,52]) {
    const answer = await observe('target', 'base', 'tsconfig.json', `chain-${count}`);
    assert.equal(answer.child?.summary ?? null, count <= 50 ? '"ready"' : null, `chain-${count}`);
  }
  for (const count of [5,6,7]) {
    const answer = await observe('target', 'base', 'tsconfig.json', `repeat-${count}`);
    assert.equal(answer.child?.summary ?? null, count === 5 ? 'number | "done"' : null, `repeat-${count}`);
  }
});

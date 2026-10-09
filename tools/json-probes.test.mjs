import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { FIXTURES, fixtureFiles, readFixture, runFixture } from './json-probes.mjs';
import { loadSemanticApi } from './semantic-oracle.mjs';

// Real-compiler tests are skipped, with this reason, unless both compilers are configured.
const compilerUnavailable = (!process.env.TYPE_SCRIPT_SEMANTIC_API || !process.env.TYPE_SCRIPT_NATIVE)
  && 'TYPE_SCRIPT_SEMANTIC_API and TYPE_SCRIPT_NATIVE are not both set';

const KINDS = new Set(['Io', 'Config', 'Syntax', 'Binding', 'UnresolvedImport', 'Unsupported']);
const ANSWERS = new Set(['unresolved', 'unsupported', 'declared']);

function temporary(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-json-probes-test-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  return directory;
}

test('every maintained probe records the compiler answer and states any tool expectation precisely', () => {
  const files = fixtureFiles();
  assert.ok(files.length > 0, `no fixtures under ${FIXTURES}`);
  for (const file of files) {
    const fixture = readFixture(file);
    for (const spec of fixture.cases) {
      const where = `${path.basename(file)} ${spec.name}`;
      const native = spec.native;
      assert.ok(native && Array.isArray(native.program) && Array.isArray(native.diagnostics), `${where}: no recorded answer`);
      if (spec.watch) assert.ok(Array.isArray(native.names), `${where}: watched names without answers`);
      const tool = spec.tool;
      if (!tool) continue;
      const unknown = Object.keys(tool).filter(key => !['error', 'names', 'note'].includes(key));
      assert.deepEqual(unknown, [], `${where}: unknown tool fields`);
      if (tool.error) {
        const [kind, place] = tool.error;
        assert.ok(KINDS.has(kind), `${where}: unknown error kind ${kind}`);
        if (place.startsWith('TS')) {
          assert.ok(native.diagnostics.some(d => d.startsWith(`${place} `) && !d.endsWith(' -')),
            `${where}: ${place} is not recorded with a position`);
        } else {
          assert.match(place, /^[^:]+:\d+:\d+$/, `${where}: ${place} is not file:line:column`);
          assert.ok(tool.note || native.diagnostics.some(d => d.endsWith(` ${place}`)),
            `${where}: a position the compiler does not report needs a note`);
        }
      }
      if (tool.names) {
        assert.ok(tool.note, `${where}: an answer that departs from the compiler's needs a note`);
        const recorded = new Set((native.names ?? []).map(name => name.split(' ')[0]));
        for (const [place, answer] of Object.entries(tool.names)) {
          assert.ok(ANSWERS.has(answer), `${where}: unknown answer ${answer}`);
          assert.ok(recorded.has(place), `${where}: no watched name at ${place}`);
        }
      }
    }
  }
});

test('fixture shape errors name the file and case', t => {
  const directory = temporary(t);
  const write = (name, fixture) => {
    const file = path.join(directory, name);
    fs.writeFileSync(file, JSON.stringify(fixture));
    return file;
  };
  assert.throws(() => readFixture(write('a.json', { cases: [] })), /missing description/);
  assert.throws(() => readFixture(write('b.json', { description: 'x', cases: [] })), /no cases/);
  const twice = { name: 'x', files: { 'tsconfig.json': '{}', 'main.ts': '' } };
  assert.throws(() => readFixture(write('c.json', { description: 'x', cases: [twice, twice] })), /repeated case name x/);
  assert.throws(() => readFixture(write('d.json', { description: 'x', cases: [{ name: 'y', files: { 'main.ts': '' } }] })),
    /y has no configuration/);
});

test('real compiler: every maintained probe answer agrees with TypeScript 7.0.2', { skip: compilerUnavailable }, async () => {
  const semantic = await loadSemanticApi();
  for (const file of fixtureFiles()) {
    const differences = runFixture(semantic, file);
    assert.deepEqual(differences.map(d => `${d.file} ${d.name}`), [], `${path.basename(file)} differs from the compiler`);
  }
});

test('real compiler: a wrong recorded answer is reported, and update rewrites only it', { skip: compilerUnavailable }, async t => {
  const semantic = await loadSemanticApi();
  const directory = temporary(t);
  const source = readFixture(path.join(FIXTURES, 'admission.json'));
  const cases = source.cases.filter(spec => ['arbitrary-omitted', 'arbitrary-true'].includes(spec.name));
  const right = structuredClone(cases[1].native);
  cases[1].native = { ...right, program: ['main.ts'] };
  const file = path.join(directory, 'probe.json');
  fs.writeFileSync(file, JSON.stringify({ ...source, cases }));
  const differences = runFixture(semantic, file);
  assert.deepEqual(differences.map(d => d.name), ['arbitrary-true']);
  assert.deepEqual(differences[0].native, right);
  assert.deepEqual(runFixture(semantic, file, { update: true }).map(d => d.name), ['arbitrary-true']);
  assert.deepEqual(readFixture(file).cases.map(spec => spec.native), [cases[0].native, right]);
  assert.deepEqual(runFixture(semantic, file), []);
});

import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { VENDORED, sha256, isLibrary, catalogFromGo, compareVendored, provenanceText } from './vendor-typescript-libs.mjs';

// Checks against the pinned native compiler are skipped, with this reason, unless it is configured.
const nativeUnavailable = !process.env.TYPE_SCRIPT_NATIVE && 'TYPE_SCRIPT_NATIVE is not set';
const provenance = () => JSON.parse(fs.readFileSync(path.join(VENDORED, 'provenance.json'), 'utf8'));

function temporary(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-libraries-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  return directory;
}

// `tsc --listFilesOnly` or `--explainFiles` for a project with one root and `options`.
function compile(directory, options, ...args) {
  fs.writeFileSync(path.join(directory, 'a.ts'), 'export {}\n');
  fs.writeFileSync(path.join(directory, 'tsconfig.json'), JSON.stringify({ compilerOptions: options, files: ['a.ts'] }));
  return spawnSync(process.env.TYPE_SCRIPT_NATIVE, ['-p', path.join(directory, 'tsconfig.json'), ...args],
    { encoding: 'utf8', timeout: 60_000 }).stdout;
}

test('the vendored directory holds exactly the files its provenance records', () => {
  const record = provenance();
  assert.equal(record.version, '7.0.2');
  assert.equal(record.upstream.tag, 'typescript/v7.0.2');
  const present = fs.readdirSync(VENDORED).filter(name => name !== 'provenance.json').sort();
  assert.deepEqual(present, Object.keys(record.files).sort());
  for (const name of present) assert.equal(sha256(fs.readFileSync(path.join(VENDORED, name))), record.files[name], name);
  assert.equal(present.filter(isLibrary).length, 108);
  assert.equal(record.libMap.length, 107);
  // The record is in the generator's canonical form.
  assert.equal(fs.readFileSync(path.join(VENDORED, 'provenance.json'), 'utf8'), provenanceText(record));
});

test('the catalog reader takes LibMap order and the target defaults from enummaps.go', () => {
  const text = `var LibMap = collections.NewOrderedMapFromList([]collections.MapEntry[string, any]{
${Array.from({ length: 107 }, (_, index) => `\t{Key: "k${index}", Value: "lib.k${index}.d.ts"},`).join('\n')}
})
var targetToLibMap = map[core.ScriptTarget]string{
\tcore.ScriptTargetESNext: "lib.esnext.full.d.ts",
\tcore.ScriptTargetES2015: "lib.es6.d.ts",
}`;
  const { libMap, targets } = catalogFromGo(text);
  assert.deepEqual(libMap.slice(0, 2), [['k0', 'lib.k0.d.ts'], ['k1', 'lib.k1.d.ts']]);
  assert.deepEqual(targets, { esnext: 'lib.esnext.full.d.ts', es2015: 'lib.es6.d.ts' });
  assert.throws(() => catalogFromGo(text.replace(/\t\{Key: "k0".*\n/, '')), /expected 107 LibMap entries/);
});

test('a damaged vendored copy is reported, not regenerated', t => {
  const directory = temporary(t);
  const files = new Map([['lib.a.d.ts', Buffer.from('a')], ['LICENSE', Buffer.from('l')]]);
  const record = { files: { 'lib.a.d.ts': sha256(files.get('lib.a.d.ts')), LICENSE: sha256(files.get('LICENSE')) } };
  for (const [name, bytes] of files) fs.writeFileSync(path.join(directory, name), bytes);
  fs.writeFileSync(path.join(directory, 'provenance.json'), provenanceText(record));
  assert.deepEqual(compareVendored({ provenance: record, files }, directory), []);
  fs.writeFileSync(path.join(directory, 'lib.a.d.ts'), 'b');
  fs.writeFileSync(path.join(directory, 'lib.extra.d.ts'), '');
  fs.rmSync(path.join(directory, 'LICENSE'));
  assert.deepEqual(compareVendored({ provenance: record, files }, directory),
    ['unexpected lib.extra.d.ts', 'missing LICENSE', 'lib.a.d.ts differs']);
});

test('real compiler: the vendored libraries are the ones the native compiler reads', { skip: nativeUnavailable }, () => {
  const compilerLibraries = path.dirname(fs.realpathSync(process.env.TYPE_SCRIPT_NATIVE));
  const compiled = fs.readdirSync(compilerLibraries).filter(isLibrary).sort();
  assert.deepEqual(compiled, fs.readdirSync(VENDORED).filter(isLibrary).sort());
  for (const name of compiled)
    assert.ok(fs.readFileSync(path.join(compilerLibraries, name)).equals(fs.readFileSync(path.join(VENDORED, name))), name);
});

test('real compiler: the recorded catalog is the one the native compiler applies', { skip: nativeUnavailable }, t => {
  const directory = temporary(t);
  const record = provenance();
  // TS6046 lists the `lib` names in LibMap order.
  const message = compile(directory, { lib: ['bogus'] }, '--noEmit').split('\n').find(line => line.includes('TS6046'));
  assert.deepEqual([...message.matchAll(/'([^']+)'/g)].slice(1).map(([, name]) => name), record.libMap.map(([name]) => name));
  // Each name selects its recorded file, which `--explainFiles` attributes to it.
  for (const [name, file] of record.libMap) {
    const explained = compile(directory, { lib: [name] }, '--explainFiles', '--noEmit').split('\n');
    const at = explained.findIndex(line => line.endsWith(`/${file}`));
    const reasons = [];
    for (let line = at + 1; at >= 0 && /^\s/.test(explained[line] ?? ''); line++) reasons.push(explained[line].trim());
    assert.ok(reasons.includes(`Library '${file}' specified in compilerOptions`), `${name} -> ${file}: ${reasons}`);
  }
  // Each target's default library is in its program.
  for (const [target, file] of Object.entries({ ...record.targetLibraries, es5: record.otherTargetLibrary })) {
    const listed = compile(directory, { target }, '--listFilesOnly').split('\n').map(line => path.basename(line));
    assert.ok(listed.includes(file), `${target} -> ${file}`);
  }
});

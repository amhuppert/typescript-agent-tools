import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { test } from 'node:test';
import { catalog, generate, output } from './ts56-library-catalog.mjs';
import { locateTypeScript56, typeScript56Unavailable } from './ts56-compiler.mjs';

// Real-compiler tests are skipped, with this reason, when the pinned
// TypeScript 5.6.3 package is not found.
const compilerUnavailable = typeScript56Unavailable();

const compiler = () => {
  const { packageDir } = locateTypeScript56();
  return { packageDir, ts: createRequire(import.meta.url)(path.join(packageDir, 'lib/typescript.js')) };
};

test('the pinned library catalog is what the pinned compiler re-extracts, byte for byte', { skip: compilerUnavailable }, () => {
  const { packageDir } = compiler();
  const pinned = fs.readFileSync(output, 'utf8');
  assert.equal(pinned, generate(packageDir), 'the Rust data diverges from typescript.js; run node tools/ts56-library-catalog.mjs');
  const sha256 = crypto.createHash('sha256').update(fs.readFileSync(path.join(packageDir, 'lib/typescript.js'))).digest('hex');
  assert.match(pinned, new RegExp(`SOURCE_SHA256: &str =\\s+"${sha256}";`), 'the provenance names another typescript.js');
});

test('the catalog is 5.6.3\'s: 88 names with two repeated, 86 files, and the target defaults', { skip: compilerUnavailable }, () => {
  const { libs, libMap, defaults, otherDefault } = catalog(compiler().ts);
  assert.equal(libs.length, 88);
  assert.equal(libMap.length, 86);
  const repeated = libs.flatMap((name, index) => (libs.indexOf(name) === index ? [] : [[name, libs.indexOf(name), index]]));
  assert.deepEqual(repeated, [['esnext.array', 70, 82], ['esnext.string', 77, 84]]);
  // `new Map(libEntries)` keeps a repeated name's first place and last file.
  const files = new Map(libMap);
  assert.equal(files.get('esnext.array'), 'lib.esnext.array.d.ts');
  assert.equal(files.get('esnext.string'), 'lib.esnext.string.d.ts');
  assert.deepEqual(libMap.map(([name]) => name), [...new Set(libs)]);
  assert.deepEqual(Object.fromEntries(defaults), {
    es3: 'lib.d.ts', es5: 'lib.d.ts', es6: 'lib.es6.d.ts', es2015: 'lib.es6.d.ts',
    es2016: 'lib.es2016.full.d.ts', es2017: 'lib.es2017.full.d.ts', es2018: 'lib.es2018.full.d.ts',
    es2019: 'lib.es2019.full.d.ts', es2020: 'lib.es2020.full.d.ts', es2021: 'lib.es2021.full.d.ts',
    es2022: 'lib.es2022.full.d.ts', es2023: 'lib.es2023.full.d.ts', esnext: 'lib.esnext.full.d.ts',
  });
  assert.equal(otherDefault, 'lib.d.ts');
});

test('every cataloged file is installed with the compiler, and every library reference in one names a library', { skip: compilerUnavailable }, () => {
  const { packageDir, ts } = compiler();
  const { libMap, defaults, otherDefault } = catalog(ts);
  const names = new Map(libMap);
  const cataloged = new Set([...libMap.map(([, file]) => file), ...defaults.map(([, file]) => file), otherDefault]);
  const installed = fs.readdirSync(path.join(packageDir, 'lib')).filter(name => /^lib\..*d\.ts$/.test(name));
  assert.deepEqual([...cataloged].sort(), installed.sort());
  for (const file of cataloged) {
    const text = fs.readFileSync(path.join(packageDir, 'lib', file), 'utf8');
    for (const reference of ts.preProcessFile(text, true, true).libReferenceDirectives) {
      assert.ok(names.has(reference.fileName.toLowerCase()), `${file} references ${reference.fileName}`);
    }
  }
});

test('the extractor refuses a compiler that is not 5.6.3', t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-ts56-libraries-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  fs.mkdirSync(path.join(directory, 'lib'));
  fs.writeFileSync(path.join(directory, 'lib/typescript.js'), 'module.exports = { version: "5.6.2", libs: [], libMap: new Map() };\n');
  assert.throws(() => generate(directory), /is TypeScript 5\.6\.2, not 5\.6\.3/);
});

import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { test } from 'node:test';
import { generate, output } from './ts56-option-catalog.mjs';
import { locateTypeScript56, typeScript56Unavailable } from './ts56-compiler.mjs';

// Real-compiler tests are skipped, with this reason, when the pinned
// TypeScript 5.6.3 package is not found.
const compilerUnavailable = typeScript56Unavailable();

test('the pinned catalog is what the pinned compiler re-extracts, byte for byte', { skip: compilerUnavailable }, () => {
  const { packageDir } = locateTypeScript56();
  const pinned = fs.readFileSync(output, 'utf8');
  assert.equal(pinned, generate(packageDir), 'the Rust data diverges from typescript.js; run node tools/ts56-option-catalog.mjs');
  const sha256 = crypto.createHash('sha256').update(fs.readFileSync(path.join(packageDir, 'lib/typescript.js'))).digest('hex');
  assert.match(pinned, new RegExp(`SOURCE_SHA256: &str =\\s+"${sha256}";`), 'the provenance names another typescript.js');
});

test('the catalog\'s messages are the ones the compiler reports', { skip: compilerUnavailable }, () => {
  const { packageDir } = locateTypeScript56();
  const ts = createRequire(import.meta.url)(path.join(packageDir, 'lib/typescript.js'));
  const pinned = fs.readFileSync(output, 'utf8');
  const report = options => ts.convertCompilerOptionsFromJson(options, '/').errors
    .map(error => `TS${error.code} ${ts.flattenDiagnosticMessageText(error.messageText, '\n')}`);
  for (const declaration of ts.optionDeclarations) {
    const enumerated = declaration.type instanceof Map ? declaration
      : declaration.type === 'list' && declaration.element.type instanceof Map ? declaration.element : undefined;
    if (enumerated) {
      const invalid = enumerated === declaration ? '\u0000' : ['\u0000'];
      const [message] = report({ [declaration.name]: invalid });
      const listed = message.match(/^TS6046 Argument for '--[^']+' option must be: (.*)\.$/)?.[1];
      assert.ok(listed, `${declaration.name}: ${message}`);
      assert.ok(pinned.includes(`listed: "${listed}"`), `${declaration.name}: ${listed}`);
    }
    if (declaration.type === 'list') {
      const wrong = declaration.element.type === 'string' || declaration.element.type instanceof Map ? 1 : 'x';
      const [message] = report({ [declaration.name]: [wrong] });
      assert.match(message, new RegExp(`^TS5024 Compiler option '${declaration.element.name}' requires a value of type `), declaration.name);
      assert.ok(pinned.includes(`name: "${declaration.element.name}",`), `${declaration.name}'s element`);
    }
  }
});

test('the extractor refuses a compiler that is not 5.6.3', t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-ts56-catalog-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  fs.mkdirSync(path.join(directory, 'lib'));
  fs.writeFileSync(path.join(directory, 'lib/typescript.js'), 'module.exports = { version: "5.6.2", optionDeclarations: [], buildOpts: [] };\n');
  assert.throws(() => generate(directory), /is TypeScript 5\.6\.2, not 5\.6\.3/);
});

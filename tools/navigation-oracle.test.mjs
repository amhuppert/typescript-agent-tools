import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import {
  DocumentSymbolServer, MessageReader, documentSymbols, expectation, expectationPath, frame, languageId, normalizeSymbols,
} from './navigation-oracle.mjs';

// Real-compiler tests are skipped, with this reason, unless the native compiler is configured.
const nativeUnavailable = !process.env.TYPE_SCRIPT_NATIVE && 'TYPE_SCRIPT_NATIVE is not set';
const fixtures = fileURLToPath(new URL('../fixtures/navigation/', import.meta.url));

function temporary(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tsparse-navigation-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  return directory;
}

function executable(directory, name, script) {
  const file = path.join(directory, name);
  fs.writeFileSync(file, `#!${process.execPath}\n${script}\n`, { mode: 0o755 });
  return file;
}

// A stand-in language server: `--version` prints `version`; `--lsp --stdio`
// writes its process id to `tsc.pid`, answers initialize, then
// `documentSymbol` with `symbols` (after sending its own configuration
// request first), and ends at the end of its input (or on `exit`) with
// `exitCode` or by `exitSignal`.
// `crash` names a method at which it exits with code 3; `silent` and `fail`
// name methods it never answers or answers with an error, staying alive.
// Responses are written one byte at a time to split framing.
function fakeServer(directory, {
  version = '7.0.2', symbols = [], crash = null, silent = null, fail = null, exitCode = 0, exitSignal = null,
} = {}) {
  return executable(directory, 'tsc', `
if (process.argv[2] === '--version') { console.log('Version ${version}'); process.exit(0); }
require('fs').writeFileSync(require('path').join(__dirname, 'tsc.pid'), String(process.pid));
const symbols = ${JSON.stringify(symbols)};
function finish() {
  if (${JSON.stringify(exitSignal)}) process.kill(process.pid, ${JSON.stringify(exitSignal)});
  else process.exit(${exitCode});
}
let buffer = Buffer.alloc(0);
const pending = [];
const write = message => {
  const body = Buffer.from(JSON.stringify({ jsonrpc: '2.0', ...message }));
  const bytes = Buffer.concat([Buffer.from('Content-Length: ' + body.length + '\\r\\n\\r\\n'), body]);
  for (const byte of bytes) process.stdout.write(Buffer.from([byte]));
};
process.stdin.on('data', chunk => {
  buffer = Buffer.concat([buffer, chunk]);
  for (;;) {
    const separator = buffer.indexOf('\\r\\n\\r\\n');
    if (separator < 0) return;
    const length = Number(/Content-Length: (\\d+)/.exec(buffer.subarray(0, separator).toString())[1]);
    if (buffer.length < separator + 4 + length) return;
    const message = JSON.parse(buffer.subarray(separator + 4, separator + 4 + length).toString());
    buffer = buffer.subarray(separator + 4 + length);
    if (message.method === ${JSON.stringify(crash)}) process.exit(3);
    if (message.method === ${JSON.stringify(silent)}) continue;
    if (message.method === ${JSON.stringify(fail)}) {
      write({ id: message.id, error: { code: -32603, message: 'refused ' + message.method } });
      continue;
    }
    if (message.method === 'initialize') write({ id: message.id, result: { capabilities: {} } });
    else if (message.method === 'textDocument/documentSymbol') {
      pending.push(message.id);
      write({ id: 'ts1', method: 'workspace/configuration', params: { items: [{}, {}] } });
    } else if (message.id === 'ts1') {
      if (JSON.stringify(message.result) !== '[null,null]') process.exit(4);
      write({ id: pending.shift(), result: symbols });
    } else if (message.method === 'shutdown') {
      if ('params' in message) write({ id: message.id, error: { code: -32602, message: 'expected no params' } });
      else write({ id: message.id, result: null });
    } else if (message.method === 'exit') finish();
  }
});
process.stdin.on('end', finish);`);
}

const lsp = (name, kind, [l1, c1, l2, c2], [s1, t1, s2, t2], children = []) => ({
  name, kind, range: { start: { line: l1, character: c1 }, end: { line: l2, character: c2 } },
  selectionRange: { start: { line: s1, character: t1 }, end: { line: s2, character: t2 } }, children,
});

test('script kinds follow the extension, including TSX', () => {
  assert.equal(languageId('a.ts'), 'typescript');
  assert.equal(languageId('a.d.ts'), 'typescript');
  assert.equal(languageId('a.mts'), 'typescript');
  assert.equal(languageId('A.TSX'), 'typescriptreact');
  assert.throws(() => languageId('a.js'), /must be TypeScript/);
  assert.throws(() => languageId('a.json'), /must be TypeScript/);
});

// Text with four lines under either line-break convention.
const PLAIN = 'function outer() {\n  inner();\n\n}\n';

test('symbols convert to one-based positions and keep their nesting', () => {
  const symbols = normalizeSymbols([lsp('outer', 12, [0, 0, 3, 1], [0, 9, 0, 14], [lsp('inner', 13, [1, 2, 1, 9], [1, 2, 1, 7])])], PLAIN);
  assert.deepEqual(symbols, [{
    name: 'outer', kind: 12, range: { start: { line: 1, column: 1 }, end: { line: 4, column: 2 } },
    selection: { start: { line: 1, column: 10 }, end: { line: 1, column: 15 } },
    children: [{ name: 'inner', kind: 13, range: { start: { line: 2, column: 3 }, end: { line: 2, column: 10 } },
      selection: { start: { line: 2, column: 3 }, end: { line: 2, column: 8 } }, children: [] }],
  }]);
  assert.throws(() => normalizeSymbols(null, PLAIN), /hierarchical/);
  assert.throws(() => normalizeSymbols([{ name: 'flat', kind: 13, location: {} }], PLAIN), /DocumentSymbol/);
});

test('positions convert from the protocol\'s line breaks to ECMAScript\'s', () => {
  // The server counts lines at CR, LF, and CRLF only (ComputeLSPLineStarts);
  // tsc and this repository also break lines at U+2028 and U+2029. On the
  // protocol's line 1 here, `f` is at character 14 and `g` at 25.
  const text = 'const a = 1;\r\nconst x = 1;\u2028function f() {}\u2029function g() {}\rconst z = 2;\n';
  const [f, g, z] = normalizeSymbols([
    lsp('f', 12, [1, 13, 1, 28], [1, 22, 1, 23]),
    lsp('g', 12, [1, 29, 1, 44], [1, 38, 1, 39]),
    lsp('z', 13, [2, 6, 2, 11], [2, 6, 2, 7]),
  ], text);
  assert.deepEqual([f.range, f.selection.start], [{ start: { line: 3, column: 1 }, end: { line: 3, column: 16 } }, { line: 3, column: 10 }]);
  assert.deepEqual([g.range, g.selection.start], [{ start: { line: 4, column: 1 }, end: { line: 4, column: 16 } }, { line: 4, column: 10 }]);
  assert.deepEqual(z.selection.start, { line: 5, column: 7 });
  assert.throws(() => normalizeSymbols([lsp('beyond', 12, [9, 0, 9, 1], [9, 0, 9, 1])], text), /outside the text/);
});

test('framing handles split headers, bodies, and several messages per chunk', () => {
  const received = [];
  const reader = new MessageReader(message => received.push(message));
  const bytes = Buffer.concat([frame({ id: 1, result: 'é' }), frame({ method: 'x' })]);
  for (const byte of bytes) reader.push(Buffer.from([byte]));
  reader.push(Buffer.concat([frame({ id: 2 }), frame({ id: 3 })]));
  assert.deepEqual(received.map(message => message.id ?? message.method), [1, 'x', 2, 3]);
  assert.equal(received[0].result, 'é');
  assert.throws(() => new MessageReader(() => {}).push(Buffer.from('Bogus: 1\r\n\r\n{}')), /Malformed/);
});

test('expectations are JSON with one symbol per line', () => {
  const symbols = normalizeSymbols([lsp('a', 12, [0, 0, 1, 1], [0, 9, 0, 10], [lsp('"b"', 7, [0, 2, 0, 5], [0, 2, 0, 5])])], PLAIN);
  const text = expectation('7.0.2', symbols);
  assert.equal(text, '{\n  "compiler": "7.0.2",\n  "symbols": [\n'
    + '    {"name": "a", "kind": 12, "range": "1:1-2:2", "selection": "1:10-1:11", "children": [\n'
    + '      {"name": "\\"b\\"", "kind": 7, "range": "1:3-1:6", "selection": "1:3-1:6", "children": []}\n'
    + '    ]}\n  ]\n}\n');
  assert.deepEqual(JSON.parse(text).symbols[0].children[0].name, '"b"');
  assert.equal(expectation('7.0.2', []), '{\n  "compiler": "7.0.2",\n  "symbols": []\n}\n');
});

test('a fake server answers through the complete handshake', async t => {
  const directory = temporary(t);
  const file = path.join(directory, 'a.ts');
  fs.writeFileSync(file, 'function a() {}\n');
  const native = fakeServer(directory, { symbols: [lsp('a', 12, [0, 0, 0, 15], [0, 9, 0, 10])] });
  const result = await documentSymbols([file], { native, root: directory });
  assert.equal(result.compiler, '7.0.2');
  assert.deepEqual(result.files[file].map(symbol => [symbol.name, symbol.selection.start]), [['a', { line: 1, column: 10 }]]);
});

// Whether the stand-in server has been reaped. It records its process id in
// `tsc.pid` once running, so under load a start that fails by timing out may
// end it before it records anything. A process still starting would record
// itself soon, so a missing or empty record is awaited for a while before it
// counts as a server that ended unrecorded.
async function reaped(directory) {
  const file = path.join(directory, 'tsc.pid');
  const recorded = () => (fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '');
  for (let waited = 0; !recorded() && waited < 2000; waited += 50)
    await new Promise(resolve => setTimeout(resolve, 50));
  if (!recorded()) return true;
  try { process.kill(Number(recorded()), 0); } catch (error) { return error.code === 'ESRCH'; }
  return false;
}

test('a failed startup ends its live server before rejecting', async t => {
  // Only the silent server needs a short timeout; one that refuses answers
  // whenever it starts.
  for (const [options, timeout, pattern] of [
    [{ silent: 'initialize' }, 200, /initialize timed out after 200 ms/],
    [{ fail: 'initialize' }, 10_000, /Language server error -32603: refused initialize/],
  ]) {
    const directory = temporary(t);
    const native = fakeServer(directory, options);
    await assert.rejects(DocumentSymbolServer.start({ native, root: directory, timeout }), pattern);
    assert.ok(await reaped(directory), `${JSON.stringify(options)} left its server running`);
  }
});

test('closing reports a server that exits abnormally after shutdown', async t => {
  for (const [options, pattern] of [
    [{ exitCode: 3 }, /exited abnormally \(code 3, signal null\)/],
    [{ exitSignal: 'SIGKILL' }, /exited abnormally \(code null, signal SIGKILL\)/],
  ]) {
    const directory = temporary(t);
    const native = fakeServer(directory, options);
    const server = await DocumentSymbolServer.start({ native, root: directory, timeout: 10_000 });
    await assert.rejects(server.close(), pattern, JSON.stringify(options));
    assert.ok(await reaped(directory));
  }
  // A clean exit closes quietly, and closing again changes nothing.
  const directory = temporary(t);
  const server = await DocumentSymbolServer.start({ native: fakeServer(directory), root: directory, timeout: 10_000 });
  await server.close();
  await server.close();
  assert.ok(await reaped(directory));
});

test('a request failure survives a cleanup that also fails', async t => {
  const directory = temporary(t);
  const file = path.join(directory, 'a.ts');
  fs.writeFileSync(file, '');
  const native = fakeServer(directory, { fail: 'textDocument/documentSymbol', exitCode: 3 });
  await assert.rejects(documentSymbols([file], { native, root: directory, timeout: 10_000 }),
    /Language server error -32603: refused textDocument\/documentSymbol/);
  assert.ok(await reaped(directory));
});

test('setup and server failures throw rather than producing empty outlines', async t => {
  const directory = temporary(t);
  const file = path.join(directory, 'a.ts');
  fs.writeFileSync(file, '');
  await assert.rejects(DocumentSymbolServer.start({ native: '' }), /TYPE_SCRIPT_NATIVE/);
  const old = fakeServer(directory, { version: '7.0.1' });
  await assert.rejects(DocumentSymbolServer.start({ native: old, root: directory }), /7\.0\.2/);
  for (const crash of ['initialize', 'textDocument/documentSymbol']) {
    const native = fakeServer(directory, { crash });
    await assert.rejects(documentSymbols([file], { native, root: directory, timeout: 10_000 }), /exited/, crash);
  }
});

for (const name of fs.readdirSync(fixtures).filter(name => /\.(ts|tsx)$/.test(name)).sort()) {
  test(`real compiler: ${name} keeps its pinned document symbols`, { skip: nativeUnavailable }, async () => {
    const file = path.join(fixtures, name);
    const result = await documentSymbols([file], { root: fixtures });
    const expected = fs.readFileSync(expectationPath(file), 'utf8');
    assert.equal(expectation(result.compiler, result.files[file]), expected,
      `regenerate with node tools/navigation-oracle.mjs --write-expectations fixtures/navigation/${name}`);
  });
}

// Navigation oracle: TypeScript 7.0.2's native language server lists each
// file's hierarchical document symbols (internal/ls/symbols.go,
// `ProvideDocumentSymbols`). The synchronous semantic API has no navigation
// method, so this harness speaks the language-server protocol to
// `tsc --lsp --stdio`.
//
//   TYPE_SCRIPT_NATIVE=/path/to/typescript-<platform>-7.0.2/package/lib/tsc \
//     node tools/navigation-oracle.mjs [--pretty] FILE...
//
// The binary must report exactly `Version 7.0.2`. Setup, protocol, and
// server failures throw: a crash is never an empty outline.
//
// Output: `{ compiler, files: { <path as given>: Symbol[] } }`, where each
// symbol is `{ name, kind, range, selection, children }`. `kind` is the LSP
// `SymbolKind` number; `range` and `selection` use one-based lines and
// one-based UTF-16 columns with exclusive ends, the Rust tool's convention.
// Lines are ECMAScript's, as in `tsc` diagnostics: the protocol's positions,
// whose lines end only at CR, LF, and CRLF, are converted through the text.
//
// Protocol notes (verified against 7.0.2): `languageId` chooses the script
// kind, so `.tsx` files are opened as `typescriptreact`. The server sends
// `workspace/configuration` and `client/registerCapability` requests that
// must be answered. Declaring dynamic watcher registration keeps the server
// from starting its own file watcher, which can panic during shutdown on
// macOS. `shutdown` takes no `params` (`null` is rejected); see `close` for
// why the end of input, not `exit`, ends the server.
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { runnerArguments } from './corpus-selection.mjs';
import { SEMANTIC_VERSION, semanticCompilerVersion } from './semantic-oracle.mjs';

export const NAVIGATION_VERSION = SEMANTIC_VERSION;

// LSP SymbolKind numbers the server uses.
export const SYMBOL_KIND = Object.freeze({
  File: 1, Module: 2, Namespace: 3, Class: 5, Method: 6, Property: 7, Constructor: 9, Enum: 10,
  Interface: 11, Function: 12, Variable: 13, EnumMember: 22, TypeParameter: 26,
});

// The script kind follows `languageId`, which overrides the extension.
export function languageId(file) {
  const extension = path.extname(file).toLowerCase();
  if (extension === '.tsx') return 'typescriptreact';
  if (['.ts', '.mts', '.cts'].includes(extension)) return 'typescript';
  throw new Error(`${file}: navigation oracle files must be TypeScript (.ts, .tsx, .mts, .cts)`);
}

// A file's text as TypeScript reads it: the native compiler's `decodeBytes`
// and `sys.readFile` drop a leading byte order mark, and editors open
// documents without it, so first-line positions count from the character
// after it.
export function compilerText(text) {
  return text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
}

export function sourceText(file) {
  return compilerText(fs.readFileSync(file, 'utf8'));
}

// The UTF-16 offsets where lines start. ECMAScript lines, which `tsc` and
// this repository use, end at CR, LF, CRLF, U+2028, and U+2029; the
// protocol's (TypeScript 7.0.2's `ComputeLSPLineStarts`) only at the first
// three.
export function lineStarts(text, { ecmascript = true } = {}) {
  const starts = [0];
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '\r' && text[i + 1] === '\n') { starts.push(i + 2); i++; }
    else if (c === '\n' || c === '\r' || (ecmascript && (c === '\u2028' || c === '\u2029'))) starts.push(i + 1);
  }
  return starts;
}

// Convert zero-based protocol positions in `text` to one-based ECMAScript
// lines and UTF-16 columns.
function positions(text) {
  const protocol = lineStarts(text, { ecmascript: false });
  const ecmascript = lineStarts(text);
  return ({ line, character }) => {
    const offset = protocol[line] + character;
    if (line >= protocol.length || !(offset <= text.length))
      throw new Error(`Position ${line}:${character} is outside the text`);
    let index = ecmascript.length - 1;
    while (ecmascript[index] > offset) index--;
    return { line: index + 1, column: offset - ecmascript[index] + 1 };
  };
}

// LSP `DocumentSymbol[]` for `text` to the oracle's shape, keeping order and
// nesting.
export function normalizeSymbols(symbols, text) {
  const position = positions(text);
  const range = ({ start, end }) => ({ start: position(start), end: position(end) });
  const convert = list => {
    if (!Array.isArray(list)) throw new Error(`Expected hierarchical document symbols, received ${JSON.stringify(list)}`);
    return list.map(symbol => {
      if (!symbol.selectionRange || !Array.isArray(symbol.children))
        throw new Error(`Expected a DocumentSymbol, received ${JSON.stringify(symbol)}`);
      return { name: symbol.name, kind: symbol.kind, range: range(symbol.range), selection: range(symbol.selectionRange),
        children: convert(symbol.children) };
    });
  };
  return convert(symbols);
}

// Incremental `Content-Length` framing over a byte stream.
export class MessageReader {
  constructor(onMessage) { this.buffer = Buffer.alloc(0); this.onMessage = onMessage; }
  push(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    for (;;) {
      const separator = this.buffer.indexOf('\r\n\r\n');
      if (separator < 0) return;
      const header = this.buffer.subarray(0, separator).toString('ascii');
      const match = /^Content-Length: *(\d+)$/im.exec(header);
      if (!match) throw new Error(`Malformed message header: ${JSON.stringify(header)}`);
      const length = Number(match[1]);
      if (this.buffer.length < separator + 4 + length) return;
      const body = this.buffer.subarray(separator + 4, separator + 4 + length).toString('utf8');
      this.buffer = this.buffer.subarray(separator + 4 + length);
      this.onMessage(JSON.parse(body));
    }
  }
}

export function frame(message) {
  const body = Buffer.from(JSON.stringify({ jsonrpc: '2.0', ...message }), 'utf8');
  return Buffer.concat([Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, 'ascii'), body]);
}

// How long a server may take to exit after its input ends or SIGTERM before
// it is killed.
const EXIT_GRACE_MS = 5_000;

// One language server. `root` is its workspace folder and working directory.
// The server owns its process for its whole life: a failed start ends and
// reaps it before rejecting, and `close` reaps it and reports an abnormal
// exit, so no evidence run outlives or ignores its compiler.
export class DocumentSymbolServer {
  static async start({ native = process.env.TYPE_SCRIPT_NATIVE, root = process.cwd(), timeout = 60_000 } = {}) {
    if (!native) throw new Error(`TYPE_SCRIPT_NATIVE must name the TypeScript ${NAVIGATION_VERSION} native compiler`);
    const binary = path.resolve(native);
    const version = semanticCompilerVersion(binary);
    const server = new DocumentSymbolServer(binary, path.resolve(root), timeout, version);
    const workspace = pathToFileURL(server.root).href;
    try {
      await server.request('initialize', {
        processId: process.pid,
        rootUri: workspace,
        workspaceFolders: [{ uri: workspace, name: 'navigation-oracle' }],
        capabilities: {
          textDocument: { documentSymbol: { hierarchicalDocumentSymbolSupport: true } },
          workspace: { configuration: true, didChangeWatchedFiles: { dynamicRegistration: true } },
        },
      });
      server.notify('initialized', {});
    } catch (error) {
      // The startup failure is the result; ending the process cannot replace it.
      await server.terminate('SIGTERM').catch(() => {});
      throw error;
    }
    return server;
  }

  constructor(binary, root, timeout, version) {
    this.version = version;
    this.root = root;
    this.timeout = timeout;
    this.pending = new Map();
    this.nextId = 1;
    this.stderr = '';
    this.failure = null;
    this.child = spawn(binary, ['--lsp', '--stdio'], { cwd: root, stdio: ['pipe', 'pipe', 'pipe'] });
    this.exited = new Promise(resolve => this.child.on('close', (code, signal) => resolve({ code, signal })));
    const reader = new MessageReader(message => this.receive(message));
    this.child.stdout.on('data', chunk => {
      try { reader.push(chunk); } catch (error) { this.fail(error); }
    });
    this.child.stderr.on('data', chunk => { this.stderr = (this.stderr + chunk).slice(-4096); });
    this.child.on('error', error => this.fail(error));
    this.child.on('close', (code, signal) => {
      if (!this.closing) this.fail(new Error(`The language server exited (code ${code}, signal ${signal}): ${this.stderr.trim()}`));
    });
    this.child.stdin.on('error', error => this.fail(error));
  }

  fail(error) {
    this.failure ??= error;
    for (const { reject } of this.pending.values()) reject(this.failure);
    this.pending.clear();
  }

  send(message) {
    if (this.failure) throw this.failure;
    this.child.stdin.write(frame(message));
  }

  notify(method, params) {
    this.send(params === undefined ? { method } : { method, params });
  }

  request(method, params) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      if (this.failure) return reject(this.failure);
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`${method} timed out after ${this.timeout} ms`));
      }, this.timeout);
      this.pending.set(id, {
        resolve: value => { clearTimeout(timer); resolve(value); },
        reject: error => { clearTimeout(timer); reject(error); },
      });
      this.send(params === undefined ? { id, method } : { id, method, params });
    });
  }

  // Responses settle requests; server requests are answered; notifications
  // (diagnostics, log messages) are ignored.
  receive(message) {
    if (message.method !== undefined && message.id !== undefined) {
      const result = message.method === 'workspace/configuration' ? message.params.items.map(() => null) : null;
      this.send({ id: message.id, result });
      return;
    }
    if (message.id === undefined) return;
    const waiter = this.pending.get(message.id);
    if (!waiter) return;
    this.pending.delete(message.id);
    if (message.error) waiter.reject(new Error(`Language server error ${message.error.code}: ${message.error.message}`));
    else waiter.resolve(message.result);
  }

  // The normalized document symbols of one file, as the server parses its
  // current contents.
  async documentSymbols(file) {
    const absolute = path.resolve(file);
    const uri = pathToFileURL(absolute).href;
    const text = sourceText(absolute);
    this.notify('textDocument/didOpen', { textDocument: { uri, languageId: languageId(absolute), version: 1, text } });
    try {
      return normalizeSymbols(await this.request('textDocument/documentSymbol', { textDocument: { uri } }), text);
    } finally {
      this.notify('textDocument/didClose', { textDocument: { uri } });
    }
  }

  // End the process and wait until it is reaped: close its input, send
  // `signal` if one is given, and kill it if it is still running after
  // `EXIT_GRACE_MS`. Resolves with its exit status.
  async terminate(signal = null) {
    this.closing = true;
    this.child.stdin.end();
    const running = () => this.child.exitCode === null && this.child.signalCode === null;
    if (signal && running()) this.child.kill(signal);
    const killer = setTimeout(() => { if (running()) this.child.kill('SIGKILL'); }, EXIT_GRACE_MS);
    try { return await this.exited; } finally { clearTimeout(killer); }
  }

  // Shut the server down, reap it, and throw if it failed or exited with a
  // nonzero status or a signal. Closing again returns the first result.
  //
  // After `shutdown`, the end of the server's input ends it, rather than an
  // `exit` notification: TypeScript 7.0.2's `Server.Run` exits with status 1
  // whenever its dispatch loop handles `exit` before its read loop reaches
  // the end of input, and with 0 otherwise, so `exit` would make the status
  // a race. Ending the input alone exits with 0 every time.
  close() {
    this.closed ??= (async () => {
      let failure = this.failure;
      if (!failure) {
        try {
          await this.request('shutdown');
        } catch (error) {
          failure = error;
        }
      }
      const { code, signal } = await this.terminate(failure ? 'SIGTERM' : null);
      if (failure) throw failure;
      if (code !== 0 || signal !== null)
        throw new Error(`The language server exited abnormally (code ${code}, signal ${signal}): ${this.stderr.trim()}`);
    })();
    return this.closed;
  }
}

// Run `use` with a started server, then close it. A failure in `use` is the
// result even when closing also fails.
export async function withServer(options, use) {
  const server = await DocumentSymbolServer.start(options);
  let result;
  try {
    result = await use(server);
  } catch (error) {
    await server.close().catch(() => {});
    throw error;
  }
  await server.close();
  return result;
}

// Document symbols for several files from one server.
export function documentSymbols(files, options = {}) {
  return withServer(options, async server => {
    const result = { compiler: server.version.replace(/^Version /, ''), files: {} };
    for (const file of files) result.files[file] = await server.documentSymbols(file);
    return result;
  });
}

// The checked-in expectation for a fixture: `<file>.symbols.json`.
export const expectationPath = file => `${file}.symbols.json`;

// JSON with one symbol per line, ranges as `LINE:COLUMN-LINE:COLUMN`, so a
// compiler change shows as a readable diff.
export function expectation(compiler, symbols) {
  const span = ({ start, end }) => `${start.line}:${start.column}-${end.line}:${end.column}`;
  const lines = (list, indent) => list.map((symbol, index) => {
    const head = `${indent}{"name": ${JSON.stringify(symbol.name)}, "kind": ${symbol.kind}, "range": "${span(symbol.range)}", `
      + `"selection": "${span(symbol.selection)}", "children": [`;
    const comma = index + 1 < list.length ? ',' : '';
    if (!symbol.children.length) return `${head}]}${comma}`;
    return `${head}\n${lines(symbol.children, `${indent}  `)}\n${indent}]}${comma}`;
  }).join('\n');
  const body = symbols.length ? `\n${lines(symbols, '    ')}\n  ` : '';
  return `{\n  "compiler": ${JSON.stringify(compiler)},\n  "symbols": [${body}]\n}\n`;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  try {
    const { flags, paths } = runnerArguments(process.argv.slice(2), {
      options: ['--pretty', '--write-expectations'], positional: Infinity,
    });
    if (!paths.length) throw new Error('Usage: node tools/navigation-oracle.mjs [--pretty | --write-expectations] FILE...');
    const result = await documentSymbols(paths);
    if (flags.has('--write-expectations')) {
      for (const [file, symbols] of Object.entries(result.files))
        fs.writeFileSync(expectationPath(file), expectation(result.compiler, symbols));
    } else {
      process.stdout.write(`${JSON.stringify(result, null, flags.has('--pretty') ? 2 : 0)}\n`);
    }
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}

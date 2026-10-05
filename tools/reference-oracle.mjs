import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import path from 'node:path';

const require = createRequire(import.meta.url);

// The JSON schema and TS6-to-TS7 adjustments are defined against this API.
export function loadCompilerApi(module = process.env.TYPE_SCRIPT_API ?? 'typescript') {
  const ts = require(module);
  if (ts.version !== '6.0.3') throw new Error(`Expected compiler API 6.0.3, received ${ts.version}`);
  return ts;
}

export function nativeCompilerVersion(binary) {
  const result = spawnSync(binary, ['--version'], { encoding: 'utf8', timeout: 10_000 });
  if (result.error || result.signal || result.status !== 0 || !/^Version 7\.\d+\.\d+(?:[-+\w.]*)?\s*$/.test(result.stdout)) {
    throw new Error(`TYPE_SCRIPT_NATIVE must point to a working TypeScript 7 compiler: ${result.error?.message || result.stderr?.trim() || result.stdout?.trim() || `exit ${result.status}, signal ${result.signal}`}`);
  }
  return result.stdout.trim();
}

// Classify the TS6 tree for TS7 syntax validation. Runners decide whether an
// ineligible fixture is an error or an exclusion; eligible invalid inputs must
// be compared as syntax rejections. Native checks use only strictDiagnostics.
export function classifyReference(ts, reference) {
  let removed;
  const strictDiagnostics = [];
  const pending = [reference];
  while (pending.length) {
    const node = pending.pop();
    if (ts.isModuleDeclaration(node) && !ts.isStringLiteral(node.name)
      && !(node.flags & (ts.NodeFlags.Namespace | ts.NodeFlags.GlobalAugmentation | ts.NodeFlags.NestedNamespace))) {
      removed ??= 'unquoted module';
      strictDiagnostics.push({ code: 1540, message: 'Unquoted module declarations were removed in TypeScript 7.' });
    }
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node) || ts.isImportTypeNode(node))
      && node.attributes?.token === ts.SyntaxKind.AssertKeyword) removed ??= 'assert attributes';
    if (node.kind === ts.SyntaxKind.JSDocUnknownType || node.kind === ts.SyntaxKind.JSDocFunctionType)
      removed ??= 'legacy JSDoc type syntax';
    // The API and native --noCheck leave this grammar error to the checker.
    if (ts.isThrowStatement(node) && ts.isIdentifier(node.expression)
      && !node.expression.escapedText && node.expression.pos === node.expression.end)
      strictDiagnostics.push({ code: 1142, message: 'Line break not permitted after throw.' });
    ts.forEachChild(node, child => { pending.push(child); });
  }
  const diagnostics = reference.parseDiagnostics.filter(d => d.code !== 2458)
    .map(d => ({ code: d.code, message: ts.flattenDiagnosticMessageText(d.messageText, '\n') }));
  diagnostics.push(...strictDiagnostics);
  return { eligible: !removed, removed, valid: !diagnostics.length, diagnostics, strictDiagnostics };
}

// Native exits do not use the Rust CLI's 0/1/2 contract. A nonzero normal exit
// requires file diagnostics; global/setup errors never establish rejection.
export function nativeDiagnostics(result) {
  if (result.error || result.signal || result.status === null)
    throw new Error(`Native process failed: ${result.error?.message || result.signal || 'no exit status'}`);
  const diagnostics = [];
  const output = [result.stdout, result.stderr].filter(Boolean).join('\n');
  for (const match of output.matchAll(/^(?:(.+?)\((\d+),(\d+)\): )?error TS(\d+): ([^\n]*)/gm)) {
    if (!match[1]) throw new Error(`Native compiler setup failed: ${match[0]}`);
    diagnostics.push({ file: path.resolve(match[1]), line: Number(match[2]), column: Number(match[3]), code: Number(match[4]), message: match[5] });
  }
  if (result.status !== 0 && !diagnostics.length)
    throw new Error(`Native compiler failed without file diagnostics: ${output.trim() || `exit ${result.status}`}`);
  return diagnostics;
}

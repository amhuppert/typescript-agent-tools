// Compare `tsparse --outline --json --depth all` with TypeScript 7.0.2's
// document symbols (tools/navigation-oracle.mjs) for TypeScript files.
//
//   TYPE_SCRIPT_NATIVE=/path/to/typescript-<platform>-7.0.2/package/lib/tsc \
//   TYPE_SCRIPT_API=/path/to/typescript-6.0.3/package/lib/typescript.js \
//     node tools/navigation-conformance.mjs [--report=PATH] FILE_OR_DIRECTORY...
//   node tools/navigation-conformance.mjs --write-outlines FILE...
//
// `TSPARSE` selects the built CLI (default `target/debug/tsparse`); the
// report defaults to `.cache/navigation-conformance.json`. Directories are
// searched for .ts, .tsx, .mts, and .cts files outside `node_modules`.
// `--write-outlines` instead pins each file's catalog entries in
// `<file>.outline.json`, which tests/navigation_cli.rs keeps current and
// tools/navigation-conformance.test.mjs mutates.
//
// Entries are matched by selection position (the compiler's selection start,
// the tool's anchor) and label. Every catalog entry's kind must name the
// syntax at its range (the fine vocabulary: `type` is a type alias, `import`
// an import, and so on). Matched entries must agree on the compiler's
// SymbolKind, through the mapping below; on range, which is the compiler's
// except that a variable binding shows its statement and an import or export
// specifier its declaration, which must contain the compiler's range; and on
// containment: the tool's nearest matched ancestor must match the compiler's.
// Every other difference must be one of the deliberate differences in
// docs/navigation.md, each counted by its code. No code is inherited: each
// entry the compiler lists without a catalog counterpart is explained by its
// own syntax, so the descendants of an omitted entry must still match.
//
//   Compiler entries the catalog omits:
//   shorthand    A shorthand property or spread, listed as a leaf. Its
//                contents, which the catalog lists, are `transparent`.
//   expando      An `f.x = …` expando entry, an omitted wrapper that
//                mergeExpandos moves under `f`. The entries inside it must
//                match, contained where the assignment's syntax is.
//   untraversed  An entry inside a parameter, type, type parameter,
//                decorator, or heritage clause, which the catalog does not
//                traverse; every entry inside one is untraversed too.
//   jsdoc        A JSDoc `@typedef` or `@callback` tag, whose range lies inside
//                a JSDoc comment. Such tags declare nothing in TypeScript.
//   merged       A copy of an entry listed elsewhere (same label, kind, and
//                ranges) inside a sibling that a catalog entry merged into:
//                mergeExpandos merges a property into every same-named
//                sibling, so its contents appear under each.
//   Catalog entries the compiler omits:
//   transparent  An entry inside a spread's expression or a shorthand
//                property's default, which the compiler does not traverse.
//   merged       A property-like declaration (not a shorthand), or a later
//                namespace, ambient module, or `declare global` block, that
//                the compiler merged into a same-named sibling; containment
//                that moved with it is merged too.
//   Matched entries:
//   truncated    A label the compiler cut at 150 characters plus `...`.
// Anything else, including a file only one side accepts, is unexplained and
// fails the run. The compiler's syntax (TypeScript 6.0.3, the repository's
// syntax baseline) classifies entries by the nodes at their ranges.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { runnerArguments, requireComparisons } from './corpus-selection.mjs';
import { loadCompilerApi } from './reference-oracle.mjs';
import { SYMBOL_KIND, lineStarts, sourceText, withServer } from './navigation-oracle.mjs';

const TS_EXTENSIONS = ['.ts', '.tsx', '.mts', '.cts'];

// The compiler's SymbolKind for each catalog kind (getSymbolKindFromNode).
// Signatures differ by label: call `()`, construct `new()`, index `[]`.
export function compilerKind(entry) {
  switch (entry.kind) {
    case 'function': return SYMBOL_KIND.Function;
    case 'variable': case 'import': case 'export': return SYMBOL_KIND.Variable;
    case 'class': case 'type': return SYMBOL_KIND.Class;
    case 'interface': return SYMBOL_KIND.Interface;
    case 'enum': return SYMBOL_KIND.Enum;
    case 'namespace': case 'module': return SYMBOL_KIND.Namespace;
    case 'method': return SYMBOL_KIND.Method;
    case 'accessor': case 'property': return SYMBOL_KIND.Property;
    case 'constructor': return SYMBOL_KIND.Constructor;
    case 'enum_member': return SYMBOL_KIND.EnumMember;
    case 'signature':
      return entry.label === '()' ? SYMBOL_KIND.Method : entry.label === 'new()' ? SYMBOL_KIND.Constructor : SYMBOL_KIND.Property;
    default: throw new Error(`Unknown catalog kind ${entry.kind}`);
  }
}

// The syntax kind names each catalog kind's range may hold. Variable
// bindings show their statement (a loop's declaration list, or a `catch`
// clause's lone declaration), and import and export specifiers their whole
// declaration.
const CATALOG_SYNTAX = {
  function: ['FunctionDeclaration', 'FunctionExpression', 'ArrowFunction'],
  class: ['ClassDeclaration', 'ClassExpression'],
  interface: ['InterfaceDeclaration'],
  type: ['TypeAliasDeclaration'],
  enum: ['EnumDeclaration'],
  enum_member: ['EnumMember'],
  namespace: ['ModuleDeclaration'],
  module: ['ModuleDeclaration'],
  method: ['MethodDeclaration', 'MethodSignature'],
  accessor: ['GetAccessor', 'SetAccessor'],
  constructor: ['Constructor'],
  property: ['PropertyDeclaration', 'PropertySignature', 'PropertyAssignment', 'Parameter'],
  variable: ['VariableStatement', 'VariableDeclarationList', 'VariableDeclaration'],
  import: ['ImportDeclaration', 'ImportEqualsDeclaration'],
  export: ['ExportDeclaration', 'ExportAssignment'],
};
const SIGNATURE_SYNTAX = { '()': 'CallSignature', 'new()': 'ConstructSignature', '[]': 'IndexSignature' };
// The statements a binding or specifier shows in place of its own range.
const STATEMENTS = ['VariableStatement', 'VariableDeclarationList', 'ImportDeclaration', 'ExportDeclaration'];
// Catalog entries that may be merged away: non-shorthand property-like
// declarations (the compiler's Property kind), and namespaces, ambient
// modules, and `declare global` blocks (its Namespace kind).
const MERGEABLE_PROPERTIES = ['PropertyDeclaration', 'PropertySignature', 'PropertyAssignment', 'Parameter', 'GetAccessor', 'SetAccessor'];

// Pre-order list of `{ side, label, kind, at, range, parent, children, raw }`.
function flatten(entries, side, toNode) {
  const nodes = [];
  const stack = entries.map(entry => [entry, null]).reverse();
  while (stack.length) {
    const [raw, parent] = stack.pop();
    const node = { side, parent, children: [], matched: null, allowance: null, ...toNode(raw) };
    if (parent) parent.children.push(node);
    nodes.push(node);
    for (const child of [...raw.children].reverse()) stack.push([child, node]);
  }
  return nodes;
}

const key = position => `${position.line}:${position.column}`;
const compare = (a, b) => a.line - b.line || a.column - b.column;
const sameRange = (a, b) => compare(a.start, b.start) === 0 && compare(a.end, b.end) === 0;
const contains = (outer, inner) => compare(outer.start, inner.start) <= 0 && compare(inner.end, outer.end) <= 0;
const span = ({ start, end }) => `${key(start)}-${key(end)}`;

const ours = entry => ({
  label: entry.label, kind: compilerKind(entry), catalogKind: entry.kind, at: entry.anchor, range: entry.range,
});
const theirs = symbol => ({ label: symbol.name, kind: symbol.kind, at: symbol.selection.start, range: symbol.range });

// Syntax facts entries are classified by: the TypeScript 6.0.3 nodes whose
// first token and end are an entry's range.
export class Syntax {
  constructor(ts, file, text) {
    this.ts = ts;
    this.starts = lineStarts(text);
    const kind = /\.tsx$/i.test(file) ? ts.ScriptKind.TSX : ts.ScriptKind.TS;
    this.source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, kind);
    this.byRange = new Map();
    this.jsdoc = [];
    const stack = [this.source];
    while (stack.length) {
      const node = stack.pop();
      if (node !== this.source) {
        const range = `${node.getStart(this.source)}:${node.end}`;
        if (!this.byRange.has(range)) this.byRange.set(range, []);
        this.byRange.get(range).push(node);
      }
      for (const comment of node.jsDoc ?? []) this.jsdoc.push([comment.pos, comment.end]);
      ts.forEachChild(node, child => { stack.push(child); });
    }
  }

  offset(position) { return this.starts[position.line - 1] + position.column - 1; }

  nodes(range) { return this.byRange.get(`${this.offset(range.start)}:${this.offset(range.end)}`) ?? []; }

  // Whether `node` has one of the named syntax kinds. Names map forward to
  // kinds; the reverse map would give aliases such as `FirstStatement`.
  is(node, names) { return names.some(name => node.kind === this.ts.SyntaxKind[name]); }

  // Why the catalog has no entry for a compiler entry, or null.
  absence(entry) {
    const ts = this.ts;
    const start = this.offset(entry.range.start), end = this.offset(entry.range.end);
    if (this.jsdoc.some(([from, to]) => from <= start && end <= to)) return 'jsdoc';
    const nodes = this.nodes(entry.range);
    if (!nodes.length) return null;
    if (nodes.some(node => ts.isShorthandPropertyAssignment(node) || ts.isSpreadAssignment(node))) return 'shorthand';
    const access = node => ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node);
    const assignment = node => ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.EqualsToken
      && access(node.left);
    // The outer expando entry is the assignment's target.
    if (nodes.some(node => assignment(node) || (access(node) && assignment(node.parent) && node.parent.left === node)))
      return 'expando';
    for (let node = nodes[0].parent; node; node = node.parent) {
      if (ts.isParameter(node) || ts.isDecorator(node) || ts.isHeritageClause(node) || ts.isTypeParameterDeclaration(node)
        || ts.isTypeNode(node)) return 'untraversed';
    }
    return null;
  }

  // Whether the catalog kind of a tool entry names the syntax at its range.
  kindAgrees(entry) {
    const expected = entry.catalogKind === 'signature' ? [SIGNATURE_SYNTAX[entry.label]] : CATALOG_SYNTAX[entry.catalogKind];
    return this.nodes(entry.range).some(node => {
      if (!this.is(node, expected)) return false;
      if (!this.ts.isModuleDeclaration(node)) return true;
      const module = this.ts.isStringLiteral(node.name) || (node.flags & this.ts.NodeFlags.GlobalAugmentation) !== 0;
      return module === (entry.catalogKind === 'module');
    });
  }

  // Whether a tool entry's range is the statement or declaration it shows
  // in place of the compiler's narrower range: a variable statement, a
  // loop's declaration list, a `catch` clause's declaration (for a binding
  // in its pattern), or an import or export declaration.
  statement(entry) {
    return ['variable', 'import', 'export'].includes(entry.catalogKind)
      && this.nodes(entry.range).some(node => this.is(node, STATEMENTS)
        || (this.ts.isVariableDeclaration(node) && this.ts.isCatchClause(node.parent)));
  }

  // Whether a tool entry is syntax the compiler merges into a same-named
  // sibling: a non-shorthand property-like declaration, or a namespace.
  mergeable(entry) {
    const kinds = entry.kind === SYMBOL_KIND.Namespace ? ['ModuleDeclaration']
      : entry.kind === SYMBOL_KIND.Property ? MERGEABLE_PROPERTIES : [];
    return this.nodes(entry.range).some(node => this.is(node, kinds));
  }

  // Whether a tool entry lies inside a spread's expression or a shorthand
  // property's default, whose contents the compiler does not list.
  transparent(entry) {
    const ts = this.ts;
    const nodes = this.nodes(entry.range);
    if (!nodes.length) return false;
    for (let node = nodes[0].parent; node; node = node.parent)
      if (ts.isSpreadAssignment(node) || ts.isShorthandPropertyAssignment(node)) return true;
    return false;
  }
}

// Compare one file's catalog entries with the compiler's symbols.
export function compareOutline({ entries, symbols, syntax }) {
  const mine = flatten(entries, 'tool', ours);
  const compiler = flatten(symbols, 'compiler', theirs);
  const index = new Map();
  for (const node of compiler) {
    const at = key(node.at);
    if (!index.has(at)) index.set(at, []);
    index.get(at).push(node);
  }
  const allowances = {};
  const differences = [];
  const checked = { kinds: 0, ranges: 0 };
  const allow = (node, code) => { node.allowance = code; allowances[code] = (allowances[code] ?? 0) + 1; };
  for (const node of mine) {
    checked.kinds++;
    if (!syntax.kindAgrees(node))
      differences.push({ kind: 'syntax', label: node.label, at: node.at, tool: node.catalogKind, range: span(node.range) });
    const candidates = (index.get(key(node.at)) ?? []).filter(other => !other.matched);
    let match = candidates.find(other => other.label === node.label);
    let truncated = false;
    if (!match) {
      match = candidates.find(other => other.label.endsWith('...') && node.label.startsWith(other.label.slice(0, -3)));
      truncated = Boolean(match);
    }
    if (!match) continue;
    node.matched = match;
    match.matched = node;
    if (truncated) allow(node, 'truncated');
    if (node.kind !== match.kind)
      differences.push({ kind: 'kind', label: node.label, at: node.at, tool: node.catalogKind, compiler: match.kind });
    checked.ranges++;
    const agrees = sameRange(node.range, match.range) || (syntax.statement(node) && contains(node.range, match.range));
    if (!agrees)
      differences.push({ kind: 'range', label: node.label, at: node.at, tool: span(node.range), compiler: span(match.range) });
  }
  // Catalog entries without a compiler entry.
  for (const node of mine) {
    if (node.matched) continue;
    if (syntax.transparent(node)) { allow(node, 'transparent'); continue; }
    const parent = matchedAncestor(node);
    const siblings = parent ? parent.matched.children : compiler.filter(other => !other.parent);
    const targets = !syntax.mergeable(node) ? [] : siblings.filter(other => other.label === node.label
      && (node.kind === SYMBOL_KIND.Namespace ? other.kind === SYMBOL_KIND.Namespace
        : [SYMBOL_KIND.Class, SYMBOL_KIND.Function, SYMBOL_KIND.Variable].includes(other.kind)));
    if (targets.length) { node.mergedInto = targets; allow(node, 'merged'); }
    else differences.push({ kind: 'extra', label: node.label, at: node.at, toolKind: node.catalogKind });
  }
  // Compiler entries without a catalog entry, each explained by its own
  // syntax, or a copy that a merge made.
  const targets = new Set(mine.flatMap(node => node.mergedInto ?? []));
  const listed = new Map();
  for (const node of compiler) {
    const identity = `${node.label} ${node.kind} ${span(node.range)} ${key(node.at)}`;
    listed.set(identity, (listed.get(identity) ?? 0) + 1);
    node.identity = identity;
  }
  for (const node of compiler) {
    if (node.matched) continue;
    const code = syntax.absence(node) ?? (listed.get(node.identity) > 1 && insideAny(node, targets) ? 'merged' : null);
    if (code) allow(node, code);
    else differences.push({ kind: 'missing', label: node.label, at: node.at, compilerKind: node.kind });
  }
  // Containment of matched entries.
  for (const node of mine) {
    if (!node.matched) continue;
    const mineParent = matchedAncestor(node)?.matched ?? null;
    const theirParent = expectedParent(node.matched, compiler);
    if (mineParent === theirParent) continue;
    if (mergedBetween(node).flatMap(merged => merged.mergedInto).includes(theirParent)) allow(node, 'merged');
    else differences.push({ kind: 'containment', label: node.label, at: node.at,
      tool: mineParent ? mineParent.label : null, compiler: theirParent ? theirParent.label : null });
  }
  return { tool: mine.length, compiler: compiler.length, matched: mine.filter(node => node.matched).length, checked,
    allowances, differences };
}

function insideAny(node, ancestors) {
  for (let parent = node.parent; parent; parent = parent.parent) if (ancestors.has(parent)) return true;
  return false;
}

function matchedAncestor(node) {
  let parent = node.parent;
  while (parent && !parent.matched) parent = parent.parent;
  return parent;
}

// The compiler entry a matched compiler entry's catalog parent must match:
// its nearest matched ancestor, except that an omitted expando wrapper, which
// mergeExpandos moved under its target, stands where its assignment is, in
// the innermost entry whose range contains it.
function expectedParent(node, compiler) {
  let parent = node.parent;
  while (parent && !parent.matched) parent = parent.allowance === 'expando' ? container(parent, compiler) : parent.parent;
  return parent;
}

function container(wrapper, compiler) {
  let best = null;
  for (const other of compiler) {
    if (other === wrapper || !contains(other.range, wrapper.range) || sameRange(other.range, wrapper.range)) continue;
    if (!best || contains(best.range, other.range)) best = other;
  }
  return best;
}

function mergedBetween(node) {
  const merged = [];
  for (let parent = node.parent; parent && !parent.matched; parent = parent.parent) if (parent.mergedInto) merged.push(parent);
  return merged;
}

function collect(target, files) {
  const stat = fs.statSync(target);
  if (stat.isFile()) { files.push(target); return; }
  for (const entry of fs.readdirSync(target, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    if (entry.name === 'node_modules' || entry.name.startsWith('.')) continue;
    const full = path.join(target, entry.name);
    if (entry.isDirectory()) collect(full, files);
    else if (TS_EXTENSIONS.some(extension => entry.name.endsWith(extension))) files.push(full);
  }
}

export function toolOutline(tsparse, file) {
  const result = spawnSync(tsparse, ['--outline', '--json', '--depth', 'all', file], {
    encoding: 'utf8', maxBuffer: 1 << 30, timeout: 120_000,
  });
  if (result.error) throw result.error;
  const document = result.stdout ? JSON.parse(result.stdout) : null;
  if (!document || result.stderr) throw new Error(`${file}: tsparse failed (exit ${result.status}): ${result.stderr.trim()}`);
  return document;
}

// The checked-in catalog entries of a fixture: `<file>.outline.json`.
export const outlinePath = file => `${file}.outline.json`;

// JSON with one entry per line, children nested below their parent, so a
// catalog change shows as a readable diff.
export function outlineExpectation(entries) {
  const lines = (list, indent) => list.map((entry, index) => {
    const { children, ...fields } = entry;
    const head = `${indent}${JSON.stringify(fields).slice(0, -1)}, "children": [`;
    const comma = index + 1 < list.length ? ',' : '';
    if (!children.length) return `${head}]}${comma}`;
    return `${head}\n${lines(children, `${indent}  `)}\n${indent}]}${comma}`;
  }).join('\n');
  const body = entries.length ? `\n${lines(entries, '    ')}\n  ` : '';
  return `{\n  "entries": [${body}]\n}\n`;
}

export async function runConformance(targets, { tsparse, ts = loadCompilerApi(), server } = {}) {
  const files = [];
  for (const target of targets) collect(path.resolve(target), files);
  const report = { files: [], summary: { files: 0, tool: 0, compiler: 0, matched: 0, checked: { kinds: 0, ranges: 0 },
    allowances: {}, differences: 0 } };
  for (const file of files) {
    const symbols = await server.documentSymbols(file);
    const document = toolOutline(tsparse, file);
    let comparison;
    if (document.outcome !== 'ok') {
      comparison = { tool: 0, compiler: symbols.length, matched: 0, checked: { kinds: 0, ranges: 0 }, allowances: {},
        differences: [{ kind: 'rejected', error: document.error }] };
    } else {
      comparison = compareOutline({ entries: document.entries, symbols, syntax: new Syntax(ts, file, sourceText(file)) });
    }
    report.files.push({ file, ...comparison });
    const summary = report.summary;
    summary.files++;
    summary.tool += comparison.tool;
    summary.compiler += comparison.compiler;
    summary.matched += comparison.matched;
    summary.checked.kinds += comparison.checked.kinds;
    summary.checked.ranges += comparison.checked.ranges;
    summary.differences += comparison.differences.length;
    for (const [code, count] of Object.entries(comparison.allowances)) summary.allowances[code] = (summary.allowances[code] ?? 0) + count;
  }
  return report;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  try {
    const { flags, values, paths } = runnerArguments(process.argv.slice(2), {
      options: ['--report=', '--write-outlines'], positional: Infinity,
    });
    if (!paths.length) {
      throw new Error('Usage: node tools/navigation-conformance.mjs [--report=PATH] FILE_OR_DIRECTORY...\n'
        + '       node tools/navigation-conformance.mjs --write-outlines FILE...');
    }
    const tsparse = process.env.TSPARSE ?? path.resolve('target/debug/tsparse');
    if (flags.has('--write-outlines')) {
      for (const file of paths) {
        const document = toolOutline(tsparse, file);
        if (document.outcome !== 'ok') throw new Error(`${file}: ${JSON.stringify(document.error)}`);
        fs.writeFileSync(outlinePath(file), outlineExpectation(document.entries));
      }
      process.exit(0);
    }
    const report = await withServer({}, server => runConformance(paths, { tsparse, server }));
    requireComparisons(report.summary.files);
    const output = values.get('--report=') ?? '.cache/navigation-conformance.json';
    fs.mkdirSync(path.dirname(output), { recursive: true });
    fs.writeFileSync(output, `${JSON.stringify(report, null, 2)}\n`);
    for (const file of report.files) {
      for (const difference of file.differences) console.log(`${file.file}: ${JSON.stringify(difference)}`);
    }
    const { summary } = report;
    console.log(`navigation conformance: ${summary.files} files, ${summary.compiler} compiler entries, ${summary.tool} catalog entries, `
      + `${summary.matched} matched, ${summary.checked.kinds} kinds and ${summary.checked.ranges} ranges checked, `
      + `allowances ${JSON.stringify(summary.allowances)}, ${summary.differences} unexplained differences`);
    if (summary.differences) process.exitCode = 1;
  } catch (error) {
    console.error(error.message);
    process.exitCode = 2;
  }
}

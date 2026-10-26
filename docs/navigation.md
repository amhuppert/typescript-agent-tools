# Declaration navigation

`tsparse --outline` lists the declarations of one TypeScript file, and
`tsparse --show` prints the exact source of one of them. Without `--project`
they read only the named file; with `--project TSCONFIG` they answer from the
project's captured snapshot, over the files reference queries search, through
the same local service as references. The Rust library exposes the same
operations as `typescript_parser::navigation::outline_file` and `show_file`,
and over a project's searched files as `Project::outline` and `Project::show`
(see [Library](#library)). `tsparse --test-outline` lists one test file's
`describe`, `it`, and `test` calls and hooks ([test view](#test-view)), an
addition with no TypeScript counterpart. The design is recorded in
[navigation-design.md](navigation-design.md); this document describes the
implemented behavior and where it deliberately differs from TypeScript 7.0.2.

## Commands

```sh
tsparse --outline src/runs/host.ts
tsparse --outline --depth all --json --pretty src/runs/host.ts
tsparse --outline --depth all --limit all src/runs/host.ts
tsparse --show --name createRunHost.startEligible src/runs/host.ts
tsparse --show --at 42:10 src/runs/host.ts --json -o shown.json
tsparse --outline --project tsconfig.json src/runs/host.ts
tsparse --show --project tsconfig.json --name createRunHost.startEligible
tsparse --show --project tsconfig.json --at 42:10 src/runs/host.ts --json
tsparse --show --project tsconfig.json --at /abs/src/runs/host.ts:42:10
tsparse --show --project tsconfig.json --direct --name startEligible src/runs/host.ts
tsparse --test-outline src/runs/host.test.ts
```

| Operation | Project | File | Selector | Options |
| --- | --- | --- | --- | --- |
| `--outline` | optional | exactly one named file | none | `--depth N` or `--depth all` (default 2), `--limit N` or `--limit all`, `--json`, `--pretty` with `--json`, `-o` |
| `--show` by name | optional | required without `--project`; optional with it, to narrow the search | `--name NAME` | `--limit N` or `--limit all`, `--json`, `--pretty` with `--json`, `-o` |
| `--show` by position | optional | exactly one named file, as the operand or in `--at` | `--at LINE:COLUMN FILE` or `--at FILE:LINE:COLUMN` | `--limit N` or `--limit all`, `--json`, `--pretty` with `--json`, `-o` |
| `--references` | required | required for `--at`, as for show; optional for `--name`, to narrow the candidates | exactly one of `--name` (one identifier), `--at` | `--calls`, `--no-tests`, `--file FILE`, `--limit N` or `--limit all`, `--json`, `--pretty` (alone, the indented document), `-o` ([find references](references.md)) |
| `--test-outline` | none | exactly one named file | none | `--limit N` or `--limit all`, `--json`, `--pretty` with `--json`, `-o` ([test view](#test-view)) |

`--direct` answers a project query (outline, show, or references with
`--project`) in the CLI process instead of the [service](#project-mode).
One request parser
decides validity before anything runs, and nothing starts or loads for an
invalid request. Navigation rejects, as usage errors: stdin (`-`), `--direct`
without `--project`, `--check`, `--tsx`, `--pretty` without `--json`, `--depth`
with `--show`, `--at` or `--name` with `--outline`, both or neither of `--at`
and `--name` with `--show`, a missing file, a FILE operand beside
`--at FILE:LINE:COLUMN`, more than one file, a `--limit` that is neither a
positive number nor `all`, a value
option (`--project`, `--name`, `--at`, `--depth`, `--limit`, `--output` or
`-o`) given twice, a second operation such as `--references` or a service action, and a
flag the operation does not accept, whose error lists every operation that
does. Parse and reference requests keep their established rules, including
the last value of a repeated option. Project
selection is explicit: no ancestor directory is searched for a configuration.
`--at` takes a one-based line and one-based UTF-16 column, `LINE:COLUMN` with
the FILE operand or `FILE:LINE:COLUMN` without one, the form text results
print, so a printed location can be pasted back. It is read from the right, so
FILE may contain `:`, as a Windows path does. A reference `--name` containing
`.` is a usage error pointing here: reference names are single identifiers,
and qualified names are show's containment selectors. An argument that is
not Unicode is a usage error, since a request names its file exactly. Argument
paths are relative to the working directory.

Navigation input is a `.ts`, `.tsx`, `.mts`, or `.cts` file, including
declaration files, classified by the same rule as project inputs (case-folded
on case-insensitive file systems). JSON, JavaScript, and other files are
unsupported input. A file that does not parse is a syntax error; there is no
partial outline.

The file is read as TypeScript reads it (`decodeBytes` in
`internal/vfs/internal/internal.go`): a leading UTF-8 byte order mark is not
part of the text, so first-line columns, anchors, ranges, excerpts, and
syntax-error positions count from the character after it, as `tsc` and
editors report them (`\u{FEFF}const a = 1;` has `a` at 1:7). Project loading
reads sources, configurations, and package manifests through the same rule.
Contents that are not UTF-8, including UTF-16, are unreadable (`Io`). A pipe,
device, or socket that project loading selects is refused before it is opened,
as `Io`, `PATH: not a regular file`, where TypeScript would read it. Parse
mode is unchanged: it parses the bytes it is given, mark included.

### Project mode

With `--project`, a file must be one of the project's searched files, the files
reference queries search, named as they name them: relative to the working
directory, matched lexically under the project's case policy. Dependency and
standard-library files and files outside the configuration's inputs are
`OutsideProject`, whose message names the configuration whose `files`,
`include`, and `exclude`, or those of a configuration it extends, decide which
files are searched; a searched JSON module is `Unsupported`. `--show --name`
without a file searches every searched file, and candidates across files are
ordered by file and then position. The whole project loads as for references:
a project that fails to load answers no operation, and its failure is never
replaced by an answer from the file alone.

Project queries go to the project service, the one that
answers every project query for the same configuration (its `--project` path
made absolute against the working directory, without resolving `..` or
symbolic links), started on demand and stopped with `--references-stop`. The
service checks the captured inputs before every request, whichever operation
it is, and loads a new generation when any changed: a source, the
configuration or a file it extends, a dependency or package manifest, or a
standard-library replacement package. A failed reload answers no request,
from any operation, until the input is repaired. `--direct` loads the project
in the CLI process through the same request boundary; both entries give the
same bytes on stdout and stderr and the same exit code for the same inputs.
Before either entry answers, the client makes the file operand absolute
against its working directory (`ProjectRequest::make_absolute`, for every
operation), so clients in any directory share the service and a direct answer
is the service's; a diagnostic that names the operand (`OutsideProject`,
`InvalidPosition`) spells it as the request did, and a working directory that
cannot be read fails with `Io` (exit 2) in both.
See [the service](references.md#persistent-cli-service) for its lifecycle.

### Exit codes and channels

One delivery policy applies to file navigation, direct project queries, and
service answers alike. The [CLI reference](cli-reference.md#outline) lists
the exit code of each outcome (also for [show](cli-reference.md#show) and
[test-outline](cli-reference.md#test-outline)), as `tsparse --OP --help`
does; this table gives the channel that receives it:

| Outcome | Text | JSON (`--json`) |
| --- | --- | --- |
| Outline, test view, or one declaration found | Result on stdout or `-o` | `"outcome": "ok"` / `"found"` document on stdout or `-o` |
| Ambiguous or missing selection | Diagnostic and candidate anchors, or a missed name's [import hint](#import-hint), on stderr | `"ambiguous"` / `"not_found"` document on stdout or `-o` |
| Syntax error, unsupported input, project analysis failure (`Config`, `Binding`, `UnresolvedImport`) | `path:line:column: message` on stderr | `"error"` document on stdout or `-o` |
| Unreadable file, invalid position, file outside the project | Diagnostic on stderr | `"error"` document on stdout or `-o` |
| Usage error | `CODE: message` on stderr | `"error"` document on stdout, never `-o` |
| Service transport failure (`transport.*`) | `CODE: message`, then `why` and `next` where it has them, on stderr | `"error"` document on stdout or `-o` |
| Failure to write the result | `CODE: message` on stderr | `CODE: message` on stderr |

A JSON outcome never writes stderr, so its document and exit status identify
it. A display cut ([Display limits](#display-limits)) changes neither an exit
code nor a channel. A document is built only in the format requested. Its `file` and
`error.path` hold exact paths only: a path that is not Unicode (possible on
Linux, for example under a working directory whose name is not Unicode) is
`null` there, and the error message names it in a lossy form. A text failure
never creates or replaces the `-o` file. A usage error, a transport failure
(the service could not start or be reached, or its runtime directory is not
private), and a failed write leave no outcome: each is a failure with a
category and a code from the [CLI reference](cli-reference.md#failure-codes),
whose text is `CODE: message` and whose document is
`{"operation", "outcome": "error", "error": {"category", "code", "message"}}`.
A failure with a remedy adds it: a runtime directory of this user's that is
not private prints `  why: …` and `  next: chmod 700 DIR`, which the document
gives as `why` and `next: {"argv": [...]}`, and a remedy that is not one
command, such as setting `TSPARSE_SERVICE_DIR`, prints as `  next: …` and is
`remedy` in the document. Each transport cause keeps its own code
(`transport.directory`, `transport.socket`, `transport.lock`,
`transport.startup`, `transport.timeout`, `transport.protocol`,
`transport.identity`), with `--json` as with text.
A usage failure's document goes to stdout, never `-o`, since the request
that named `-o` was not valid; a usage error about `--json` itself, or an
argument that is not Unicode, stays text. A transport failure never becomes
a direct load. A result that cannot be written is one diagnostic, never
followed by a second document. An empty file has an empty,
successful outline. Unlike reference name queries, which succeed with no
groups, a show selector that matches nothing exits 1. A position outside the
file, past the end of its line, or inside a surrogate pair is invalid (exit 2);
a valid position on no declaration name or anchor and in no statement's first
token ([statement sites](#statement-sites)) is a missing selection.

## The declaration catalog

Outline and show use one catalog of **syntax sites**, not merged symbols:
overloads, merged interfaces, and two blocks of one namespace are separate
entries. Inclusion, containment, and labels port TypeScript 7.0.2's document
symbols (`getDocumentSymbolsForChildren`, `newDocumentSymbol`, and
`getUnnamedNodeLabel` in `internal/ls/symbols.go`), apart from the differences
below. An entry's children are the entries inside it; other syntax is
transparent.

| Syntax | Kind | Children |
| --- | --- | --- |
| Function declaration; function expression or arrow that is not a variable's or property's initializer | `function` | body |
| Class declaration; class expression that is not an initializer | `class` | members (a static block is transparent) |
| Interface | `interface` | members |
| Type alias | `type` | none |
| Enum, enum member | `enum`, `enum_member` | members, none |
| `namespace A.B.C` (one entry) | `namespace` | innermost body |
| `declare module "m"`, `declare global` | `module` | body |
| Method (class or object literal), method signature | `method` | body |
| Get or set accessor | `accessor` | body |
| Constructor | `constructor` | body; parameter properties follow it as `property` siblings |
| Call, construct, and index signatures | `signature` | none |
| Property declaration, property signature, property assignment | `property` | initializer's contents |
| Variable or binding element with an identifier name | `variable` | initializer's contents |
| Default import, namespace import, import specifier, `import x =` | `import` | none |
| Export specifier; `export =` | `export` | none; the expression |

A variable declared with a binding pattern has one entry per identifier
binding and no children from its initializer. An initializer adds no entry of
its own: `const run = () => { function step() {} }` lists `step` under `run`,
and `const run = wrap(() => {})` lists `wrap() callback` under `run`. Function
bodies are traversed; parameters, types, type parameters, decorators, and
heritage clauses are not. Shorthand properties, spreads (object, array, and
argument spreads alike), `export default <expression>`, `export *`, and
assignments have no entry and are transparent: their contents belong to the
enclosing entry. `const merged = { ...{ run() { function step() {} } } }`
lists `run` under `merged` and `step` under `run`, and `{ ...build(() => {}) }`
lists `build() callback`; a shorthand property's default in a destructuring
assignment, `({ a = function fallback() {} } = value)`, lists `fallback`.

Kinds are this tool's own vocabulary, shared by text and JSON. A
function-valued variable is still a `variable`; a class arrow field or object
property is still a `property`. One more kind, `statement`, belongs only to
[statement sites](#statement-sites), which `--at` selects and which are never
outline entries.

### Labels, keys, and selectors

Each entry has a **label** for presentation and, when its name is exact, a
**key** for selection. Labels follow `getTextOfName` and
`getUnnamedNodeLabel`:

- Identifiers and private names: decoded (`caf\u00e9` is `café`, `#count`).
- String names: double-quoted and escaped as the compiler's printer does
  (`"display-name"`, `"\uD800"`); numeric names: normalized (`0x10` is `16`).
- Computed names: a literal's rendering (`["lit"]` is `"lit"`), else source
  text (`[Symbol.iterator]`).
- A dotted namespace: its chain, `A.B.C`. `export =`: `export=`.
- An anonymous default-exported function or class: `default`.
- An anonymous function passed directly to a call whose callee is a name or
  property-access chain: `name(args) callback`, with string and template
  arguments as written and line breaks removed (`describe('runs') callback`).
- A function or class expression assigned with `=`: the assigned name
  (`handler = function () {}` is `handler`).
- Otherwise `<function>`, `<class>`, `constructor`, `()`, `new()`, or `[]`.

A key exists only for a decoded identifier or private name, a string name
without an unpaired surrogate (its exact value), a numeric name (its normalized
text), and a dotted namespace (its segments). Computed names, string names with
an unpaired surrogate, assigned names, and every synthesized label have none;
such entries are selected by position. Imports and exports have keys only
where TypeScript's project-wide name search counts them as declarations
(`ProvideWorkspaceSymbols` over `computeDeclarationMap`, which `tsc --lsp`
answers as `workspace/symbol`): an import or export specifier only when it
renames (`{ a as b }` declares `b`; `{ a }` and `export { a }` declare
nothing), a namespace import and `import x =` always, and a default import's
name never. So `--name createThing` selects the function, not each file that
imports or re-exports it, while `--name made` selects
`import { createThing as made }`. A specifier without a key keeps its entry,
label, and anchor, and `--at` on its name shows its whole declaration; a
`--name` that misses its file names it ([import hint](#import-hint)).
Only imports and exports follow workspace symbols: object-literal properties
keep their keys (workspace symbols omit them), so a name shared by a
declaration and a property, often a test mock, is ambiguous and `--at`
chooses; a dotted namespace is keyed by its segments, not by its joined
name. Two different unpaired surrogates have
different labels and no keys, so they never merge with each other or with a
literal U+FFFD, whose escape `"\uFFFD"` and literal form name one property.

`--name` matches exact keys, with exact case and no fuzzy matching. A selector
is key segments joined with `.`, so it can spell exactly the segments that are
non-empty and contain no `.`; one rule decides both which selectors match and
which entries report a `selector`. Each entry's key is one segment, except a
dotted namespace's, which is its chain's segments (`namespace A.B.C` has the
segments `A`, `B`, `C`). A selector matches an entry when its segments, read
from the end, are the entry's key segments and then the key segments of its
nearest keyed ancestors, in order, ending anywhere: ancestors without a key,
such as callbacks, are skipped, and ancestors beyond the selector are
unconstrained, so `b.c` also matches inside `X.a.b.c`. Hence `C`, `B.C`, and
`A.B.C` select `namespace A.B.C`, and `C.deep` and `A.B.C.deep` select `deep`
inside it, while `A` and `A.B` select nothing, because the site declares `C`
within `A.B`. `settings.display-name`, `Secrets.string method`, and
`@scope/pkg.X` are selectors; a selector with an empty segment (`settings..e`,
`.x`, `x.`, or the empty name) matches nothing, and an entry whose key contains
`.` or is empty (the properties `"a.b"` and `""`), and every entry inside it,
has no `selector` and is selected by position. Each entry's `selector` field is its full qualified name, present
exactly when `--name` can spell it.

`--at LINE:COLUMN` selects the entries whose name token (any segment of a
dotted namespace, a constructor's `constructor` keyword, a computed name's
whole `[…]`) or anchor contains the position; a name's end is outside it. One
entry is found; several are candidates. Only when no entry matches does the
position select a [statement site](#statement-sites). An entry's **anchor** is
its name's first character, the first segment of a dotted namespace, or the
first token of an unnamed site (including decorators and modifiers). Outline
entries and ambiguity candidates always carry an anchor that `--at` selects.

When several entries match, show lists every candidate rather than choosing:
overloads, same-named locals in different blocks, merged declarations, and a
renaming import or export beside a declaration of the same name all remain
separate.

### Statement sites

When no entry matches a position, `--at` selects the statement whose first
token contains it, from the token's first character up to but not including
its end, as a **statement site**; otherwise the selection is missing, with
the text `no declaration is named or anchored at this position` and exit 1.
Every position that selects an entry keeps selecting exactly that entry or
those candidates, so a statement site never shadows an entry. This is a
deliberate addition: TypeScript's document symbols have no statements.

A statement is any node of the final syntax tree that TypeScript's
`IsStatement` accepts (`syntax::is_statement`): expression, control-flow,
variable, declaration, import, and export statements, wherever they are,
including the unbraced body of an `if`, `else`, loop, `with`, or label, and
every block except a function body and a `try`, `catch`, or `finally` block,
so a class static block's body is one. Its first token is the first token of
its range (TypeScript's `getFirstToken`, `Ast::first_token_span`): decorators
and modifiers included, leading comments and JSDoc excluded, scanned without
the parser's rescans, so a statement that begins with a regular expression or
JSX has `/`, `/=`, or `<` as its first token. A token is the first token of at
most one statement, so a statement site is selected only alone, never among
an ambiguous selection's candidates.

In this file:

```ts
 1 import { describe, it, test, beforeEach, expect } from "vitest";
 2 class Widget {}
 3 describe(Widget.name, () => {
 4   beforeEach(() => {});
 5   it.each([1, 2])("adds %i", (n) => { expect(n).toBeTruthy(); });
 …
10   it.skip("skipped one", () => {});
```

`--at 10:3` and `--at 10:4`, both in `it`, select line 10's statement, and
`--at 10:5`, the `.`, selects nothing. `--at 4:3` selects the hook's
statement and `--at 5:3` the `it.each` statement, while `--at 5:30`, the `(`
of `(n)`, is the callback entry's anchor and still selects the callback.
`--at 1:1`, on `import`, selects the import declaration (`--at 1:10`, on
`describe`, still selects that specifier, with the same excerpt), and
`--at 2:1`, on `class`, selects the class statement while `--at 2:7` selects
the class. Likewise `export` in `export { a }` or `export function f() {}`,
`function` in `function f() {}`, and `const` in `const x = 1;` select their
whole statements. A position an entry claims keeps the entry: the first token
of `x => {};` and `async () => {};` is the arrow's anchor;
`export default function () {}`, `export default class {}`, and `export = f(…)`
have entries anchored at `export`; and a statement inside a computed name,
`[(() => { f(); })()]() {}`, lies in the member's name token. `@dec export
class C {}` is one statement whose first token is `@`; its `export` and
`class` begin no statement and select nothing. In `a: b: f();` the first
tokens are `a`, `b`, and `f`, and in `if (a) if (b) f();` the two `if`s and
`f`.

A statement site is reported as a declaration of kind `statement`:

- `label`: the statement's text without a final `;`, rendered as a signature
  is: each run of whitespace and comments between tokens becomes one space,
  function block bodies and class member lists become `{ … }`, a string,
  template, or JSX text literal that spans lines becomes `…`, and past 160
  characters the text is cut at the renderer's last boundary and ends with
  `…`. With no name, the cut may fall at the start, so a statement whose first
  160 characters hold no boundary, such as a long `a.b().c()…` chain, is `…`
  alone. This is the one label that abbreviates: the rule that labels are
  never truncated protects names, and a statement site has none. An empty
  statement, `;`, has the empty label, so its text line ends `statement `
  with the space before the label.
- `anchor`: the start of its first token. `range`, and the excerpt `--show`
  prints: the statement from its first token through its last, including a
  final `;`. The `lines` display limit applies as to any excerpt.
- `depth`: one more than the greatest depth among the outline entries whose
  range contains the statement's range and differs from it, or 1. Show does
  not print it; edit documents and plans, which serialize the whole site, do.
- No `key`, `selector`, or `signature`, so `--name` never selects a statement
  site. Statement sites are kept apart from the catalog's entries: the
  outline, every entry, its place, and name selection, in a file or a
  project, are unchanged.

```text
/abs/edge-cases.test.ts:10:3 statement it.skip("skipped one", () => { … })
10 | it.skip("skipped one", () => {});
```

```json
{"declaration":{"anchor":{"column":3,"line":10},"kind":"statement","label":"it.skip(\"skipped one\", () => { … })","range":{"end":{"column":36,"line":10},"start":{"column":3,"line":10}}},"file":"/abs/edge-cases.test.ts","operation":"show","outcome":"found","source":"it.skip(\"skipped one\", () => {});"}
```

With `--project`, through the service or `--direct`, a position selects as
in the file, with files named as project results name them. Edits select
the same sites: [statement insertion](editing.md#statement-insertion-e2)
inserts after a statement site in a statement list, and its refusal at a
callback names the position that selects the callback's innermost enclosing
statement when that statement is in a statement list and the position
selects only it;
[body replacement](editing.md#body-replacement-e1) refuses one. `--type` and
`--references` select tokens by their own rules, not catalog sites, and are
unchanged.

### Ranges, excerpts, and signatures

An entry's range, which show prints, runs from its first token, including
decorators and modifiers, through its last token; leading comments, including
JSDoc, are excluded. A variable binding's range is its variable statement,
which may include sibling bindings (a loop's declaration list, or the variable
alone in a `catch` clause). An import or export specifier's range is its whole
import or export declaration. Show never truncates the excerpt.

A signature summarizes written syntax on one line, without inferred types:
the header through the parameters and return type for callables, the
declaration up to its member list for classes, interfaces, enums, and
namespaces, and the whole declaration for other entries. Whitespace and
comments between tokens collapse to one space; nested function and class bodies
become `{ … }`; a string, template, or JSX text literal that spans lines becomes
`…`; and a signature longer than 160 characters ends with `…` at a token
boundary after the name. Literal and token contents are never altered: a
literal the signature keeps is kept whole, including trailing spaces, tabs, or
NBSP in JSX text, and only the whitespace generated between tokens can end
before the `…`.
Callbacks and other anonymous function or class expressions, bindings in a
pattern, and import or export specifiers have no signature; their label
identifies them.

## Output

The text outline prints the file, then one line per shown entry, indented two
spaces per level, as `LINE:COLUMN kind signature` (or the label without a
signature). An import declaration's entries, one per imported name, take one
line at the first name's anchor, which `--at` selects, listing their labels
(the first eight, then `… N more`); the catalog and JSON keep an entry per
name. Where `--depth` hides children, a
line says how many:

```text
/abs/src/host.ts
1:8 import def, alias, Only
2:13 import ns
4:17 function export function createRunHost(deps: Deps)
  … 2 more children
10:10 function function overload(a: string): void
```

Text show prints the anchor, kind, and label, then the excerpt with its
original line numbers:

```text
/abs/src/host.ts:2:12 function startEligible
2 | function startEligible() {
3 |     return true;
4 |   }
```

JSON documents share `operation`, `outcome`, and a path base: an absolute
`file` in file navigation, and `project` in project mode. Entries carry
`label`, `kind`, `anchor`, `range`, and `children`, and `key`, `selector`, and
`signature` when present. An entry whose children the depth hides has
`"children": []` and `omitted_children`. A `found` document's `file` is the
declaration's file; an `ambiguous` or `not_found` document's `file` is the file
the selection searched, absent when a project-wide name searched every file.

```json
{"operation":"outline","outcome":"ok","file":"/abs/src/host.ts","entries":[
  {"label":"createRunHost","key":"createRunHost","selector":"createRunHost","kind":"function",
   "anchor":{"line":1,"column":17},
   "range":{"start":{"line":1,"column":1},"end":{"line":6,"column":2}},
   "signature":"export function createRunHost(deps: Deps)","children":[],"omitted_children":2}]}
{"operation":"show","outcome":"found","file":"/abs/src/host.ts","declaration":{…},"source":"run() {}"}
{"operation":"show","outcome":"ambiguous","file":"/abs/src/host.ts","query":{"name":"overload"},
 "candidates":[{"file":"/abs/src/host.ts","label":"overload","kind":"function","key":"overload",
   "selector":"overload","anchor":{"line":7,"column":10}}, …]}
{"operation":"show","outcome":"not_found","file":"/abs/src/host.ts","query":{"at":{"line":3,"column":12}}}
{"operation":"outline","outcome":"error","file":"/abs/src/broken.ts","error":{"kind":"Syntax",
 "message":"Expected a binding identifier","path":"/abs/src/broken.ts","position":{"line":1,"column":7}}}
```

A candidate's `file` and `anchor` form the next request directly:
`tsparse --show --at /abs/src/host.ts:7:10`, or
`tsparse --show --at 7:10 /abs/src/host.ts`. File navigation's error kinds are
`Io`, `Syntax`, `ResourceLimit` (a file nesting deeper than the parser's
grammar budget), `Unsupported`, and `InvalidPosition`; each kind's exit code
has one owner shared with reference commands.

In project mode, as in reference results, `project` is the configuration's
canonical path and every `file` is relative to its directory with `/`
separators (`../` for a searched file outside it). Text names files
absolutely instead: the configuration's directory joined with the relative
file, with `..` segments resolved, which is exact because that directory is
canonical. Either way, the next request works from any working directory:
join a JSON candidate's `file` to the directory of `project`, or pass a text
candidate's `FILE:LINE:COLUMN` to `--at`. A text candidate line ends with its
qualified `selector` in parentheses when it has one, so containment can choose
between candidates without opening their files. A project-wide name names the
configuration as the scope of its text diagnostics.

```json
{"operation":"outline","outcome":"ok","project":"/abs/app/tsconfig.json","file":"src/host.ts","entries":[…]}
{"operation":"show","outcome":"ambiguous","project":"/abs/app/tsconfig.json","query":{"name":"run"},
 "candidates":[{"file":"src/host.ts","label":"run","kind":"method","key":"run",
   "selector":"createRunHost.run","anchor":{"line":5,"column":27}},
  {"file":"src/other.ts","label":"run","kind":"function","key":"run","selector":"run",
   "anchor":{"line":1,"column":17}}]}
{"operation":"show","outcome":"found","project":"/abs/app/tsconfig.json","file":"src/other.ts",
 "declaration":{…},"source":"export function run() {}"}
{"operation":"outline","outcome":"error","project":"tsconfig.json","error":{"kind":"OutsideProject",
 "message":"not one of the project's searched files; the files, include, and exclude of tsconfig.json, or of a configuration it extends, decide which files are searched",
 "path":"outside/x.ts","position":null}}
```

```sh
tsparse --show --project /abs/app/tsconfig.json --at /abs/app/src/host.ts:5:27
tsparse --show --project /abs/app/tsconfig.json --name createRunHost.run
```

```text
/abs/app/tsconfig.json: `run` matches 2 declarations; select one with --at FILE:LINE:COLUMN
/abs/app/src/host.ts:5:27 method run (createRunHost.run)
/abs/app/src/other.ts:1:17 function run (run)
```

A project `error` document names the configuration as the request spelled it,
since a project that failed to load has no canonical path, and no `file`;
`error.path` locates the failure. Project error kinds add the project's
(`Config`, `Binding`, `UnresolvedImport`) and the query's (`OutsideProject`).
A diagnostic about the request's own file (`OutsideProject`,
`InvalidPosition`) names it as the request spelled it; every other failure
names the captured file by its canonical path.

### Import hint

`--show --name NAME FILE` selects declared names only, so it misses a name
FILE imports or re-exports without declaring it. Such a miss, in file mode or
project mode, keeps its first line and adds one line per specifier of FILE
whose catalog entry has kind `import` or `export`, no key, and an identifier
name decoded exactly to NAME, in source order: a default import, `import {
a }` (also `import type { a }` and `import { type a }`), `export { a } from
"m"`, and `export { a }`. A renaming specifier, a namespace import, and
`import x =` have keys, so `--name` finds them; `export * as ns`, `export *`,
and `export default x` have no entry; a string name (`export { "a-b" }`) and
`export =` have no identifier name. Matching is by syntax alone, so an
undeclared `export { a };` matches too; what a specifier denotes is
`--definition`'s answer at its anchor ([references](references.md)).

```text
/abs/src/app.ts: no declaration matches `formatTitle`
`formatTitle` is imported at /abs/src/app.ts:1:10; find its declaration with: tsparse --definition --project tsconfig.json --at /abs/src/app.ts:1:10
```

An `export` entry reads `re-exported`, and FILE is absolute in both places.
In project mode the command is complete: it repeats the request's own
`--project` spelling, quoted where a shell needs it, and names no `--direct`,
so the service's and a direct answer are byte-identical. File mode knows no
configuration, so the command names `--project`'s placeholder, `TSCONFIG`,
for the caller to fill in. The `not_found` document gains `specifiers`, one
object per line with `kind` and `anchor` (in the document's `file`), and
`next` only where the command is complete:

```json
{"file":"src/app.ts","operation":"show","outcome":"not_found","project":"/abs/tsconfig.json","query":{"name":"formatTitle"},"specifiers":[{"anchor":{"column":10,"line":1},"kind":"import","next":{"argv":["tsparse","--definition","--project","tsconfig.json","--at","/abs/src/app.ts:1:10"]}}]}
```

A miss that matches no specifier, a missing `--at`, a project-wide `--name`
without FILE, and an edit whose selection misses are unchanged. The facts
come from FILE's complete catalog: in project mode one more projection of its
captured text, only on a miss, as `--at` projects it, and a failure to
project it fails the request as that input failure.

### Display limits

Outline, show, and the test view bound what they display; the library's
results are always complete, and a cut is a rendering decision. Each list has
a default, which `tsparse --OP --help --json` publishes under `limits`:

| List | Default | Counted |
| --- | --- | --- |
| Outline `entries` | 200 | after `--depth` chooses the levels, in source order |
| Test view `entries` | 200 | in source order, every level |
| Show `lines` | 400 | from the declaration's first line |
| Show `candidates` | 50 | of an ambiguous selection, in the order they are listed |

`--limit N` (a positive number) shows at most N items of the list, in every
channel; `--limit all` shows every item. `-o` is the spillover channel: with
`-o` and no `--limit`, nothing is cut, so an excerpt or outline that reaches
`-o` is complete. Without `-o`, `--json` is cut like text and says so. A cut
keeps the first items and changes no outcome, field, or exit code: the
outline's `omitted_children` and `… N more children` lines still count what
`--depth` hides, a cut show is still `found`, and a cut ambiguity still exits
1 with every candidate counted in its first line.

A cut text ends with one line naming what it withheld and the command that
shows it: the request as given, with `--limit all` in place of any `--limit`
value, rendered so a shell reads it back exactly.

```text
… 812 more entries; see all with: tsparse --outline src/app.ts --limit all, or write them to a file with -o FILE
… 600 more lines; see all with: tsparse --show --name run src/host.ts --limit all, or write them to a file with -o FILE
… 12 more candidates; narrow with --name a.b or --at FILE:LINE:COLUMN, or see all with: tsparse --show --project tsconfig.json --name run --limit all
```

The command keeps the request's form and channel: a text outline's command
prints text, whose import lines list eight names and count the others, as
every text outline does, and the document lists one entry per name. Where
the cut falls inside an import declaration, its line lists the names before
the cut and counts the declaration's others, as it counts those past the
eighth, so the line never reads as the whole import. A cut show
prints the declaration's first N lines, numbered as the whole declaration is.
Candidates are a diagnostic on stderr, which never reaches `-o`, so their line
suggests narrowing instead of a file. A cut document adds `display`, present
only when something was withheld, so every document that shows all its items
is unchanged; a cut show's `source` is the first N lines of the excerpt,
ending before the line break after the last:

```json
"display": {"entries": {"returned": 200, "total": 1012},
            "next": {"argv": ["tsparse", "--outline", "src/app.ts", "--json", "--limit", "all"]},
            "truncated": true}
```

`next.argv` is the same command as an argument list, `tsparse` first. It
keeps the request's `-o`, so following it writes every item to the file the
request named, and it names no transport: the service and a direct load
answer alike, so it leaves out `--direct`, and the service's answer and a
direct one stay byte-identical. Followed from the same working directory, it
answers through the project's service, as every project request without
`--direct` does.

## Parsing

Navigation parses with the same policy as project loading: the deep grammar
budget (`ParseOptions::DEEP_MAX_DEPTH`, 1,024 grammar entries) on an
operation-owned thread with `DEEP_PARSE_STACK_BYTES`. Callers never choose a
budget or stack. For example, 204 nested function declarations are accepted
and 205 are a syntax error, while parse mode's default budget rejects 52.
`--depth` filters what is displayed only; it never changes which files parse
or which entries exist. The catalog is built by an iterative walk of the final
syntax tree, so deep files need no deeper native stack, and none of its
helpers recurses on input either: a callback's callee chain, which the parser
reads in a loop and so is as long as the file allows, is walked in a loop,
and a signature reads its source only until it is long enough. Loading a
project builds each searched file's index without rendering signatures or
the labels of unnamed sites, which only outlines and shows use.

## Library

```rust
use std::path::Path;
use typescript_parser::navigation::{outline_file, show_file, Selector, ShowOutcome};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let outline = outline_file(Path::new("src/host.ts"))?;
    for entry in &outline.entries {
        println!("{}{} {}", "  ".repeat(entry.depth - 1), entry.kind, entry.label);
    }
    let shown = show_file(Path::new("src/host.ts"), &Selector::Name("createRunHost".into()))?;
    if let ShowOutcome::Ambiguous(candidates) = shown.outcome {
        for candidate in candidates {
            let again = show_file(Path::new(&candidate.file), &Selector::At(candidate.anchor))?;
            assert!(matches!(again.outcome, ShowOutcome::Found(_)));
        }
    }
    Ok(())
}
```

`Outline::entries` is the complete hierarchy in source order, each entry with
its containment `depth`; display depth is the caller's choice. A
`Selector::At` that selects a [statement site](#statement-sites) finds a
`Declaration` of kind `Kind::Statement`, which no outline entry has. Failures are
`analysis::AnalysisError`s, the type every analysis operation reports, file
or project: input failures (`Io`, `Syntax`, `ResourceLimit`, `Unsupported`) are
`AnalysisError::Input` (`analysis::InputError`), and an invalid position is
`AnalysisError::Query` (`InvalidPosition`); positions are
`source::SourcePosition`s. Results and errors serialize with serde; no
syntax-tree or arena identifiers appear in them.

### Project navigation

A loaded `Project` answers the same operations over its searched files, the
files reference queries search, from the text it captured:

```rust
use std::path::Path;
use typescript_parser::navigation::{ShowOutcome, ShowQuery};
use typescript_parser::project::Project;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project = Project::load(Path::new("tsconfig.json"))?;
    let outline = project.outline(Path::new("src/host.ts"))?;
    assert_eq!(outline.file, "src/host.ts");
    let shown = project.show(ShowQuery::Name { name: "run".into(), file: None })?;
    if let ShowOutcome::Ambiguous(candidates) = shown.outcome {
        let base = shown.project.as_deref().and_then(Path::parent).unwrap();
        for candidate in candidates {
            let again = project.show(ShowQuery::At {
                file: base.join(&candidate.file),
                position: candidate.anchor,
            })?;
            assert!(matches!(again.outcome, ShowOutcome::Found(_)));
        }
    }
    Ok(())
}
```

- **Membership.** A file must be one of the project's searched files, named as
  reference queries name them (relative to the working directory, matched
  lexically under the project's case policy). Dependency, external-library,
  and default-library files, and files outside the configuration's inputs, are
  `OutsideProject`. A searched JSON module is `Unsupported` ("JSON files are
  not navigation input"), as in file navigation, naming the module by its
  canonical path, as other input failures do. A path names a file only by its
  exact text: a path that is not Unicode is `Unsupported` for every operation,
  as in file navigation, even when its lossy form, with U+FFFD for each
  invalid byte, is a searched file's name.
- **Selection.** `ShowQuery::Name` with no file searches every searched file;
  with a file it narrows the search to that file. `ShowQuery::At` needs a
  file. Name matching, positions, ambiguity, and missing selections are those
  of file navigation, and one rule decides every name match and candidate,
  in a project or a file; candidates across files are ordered by file and
  then position.
- **Paths.** A project result names its configuration file in `project`, the
  canonical path reference results report, and every `file` relative to its
  directory with `/` separators. File navigation leaves `project` absent and
  names absolute files. `Show::file` is the file a selection searched, absent
  for a project-wide name; `Found::file` and `Candidate::file` name each
  declaration's file, so a candidate's file, joined to the project's
  directory, and anchor select it with `ShowQuery::At`.
- **Snapshot and session.** A `Project` never refreshes: edits after loading
  are not seen. `ProjectSession::execute` takes a `ProjectRequest`
  (`References`, `Outline`, or `Show`), refreshes the generation when inputs
  changed, and answers through `Project::execute`; a failed refresh answers no
  operation from the earlier generation, and a project failure never falls
  back to navigating the file alone.
- **Cost.** The snapshot keeps a compact index of each searched file's named
  sites. A name search, across files or narrowed to one, reads only the
  indexes when it finds no match or several; a single match, an outline, and a
  show by position parse the selected file's captured text again with the
  navigation parsing policy below, so their cost grows with that file's size
  ([measurements](navigation-performance.md)). Through the CLI on unchanged
  Command Center, a warm service request takes 0.40–0.49 s, almost all of it
  the session's freshness validation, against 5–91 ms for file-only
  navigation of the same files ([Command Center
  performance](performance-command-center.md#current-state-2026-10-08)).

## Test view

`tsparse --test-outline FILE` lists one test file's test calls, its
`describe`, `it`, and `test` calls and hooks, in source order, indented by
containment, with each test's name. It is a deliberate addition with no
TypeScript counterpart. TypeScript's document symbols list a test only as its
callback (`it("adds") callback`), a table-driven test as `<function>`, and
`it.todo("…")` not at all, and the outline keeps those labels because it
ports them. So the test view has its own owner (`navigation::test_outline`),
which ports no TypeScript function. Its rules are not document-symbol
differences: the [table below](#differences-from-typescript-702-document-symbols)
and the conformance runner do not cover it, and `--outline` is unchanged.

FILE is [navigation input](#commands), read, classified, and parsed as for
`--outline FILE`; only that file is read, with no project and no service. The
operation takes `--limit`, `--json`, `--pretty` with `--json`, and `-o`, besides
`--help` and `--version`. Any other flag, such as `--project`, `--direct`,
`--depth`, `--at`, or `--name`, is a usage error that lists the operations
accepting it, and so is stdin (`-`).

**Test calls.** Syntax alone decides what is a test call, with no import
check, so Vitest globals count. A test call is an expression statement whose
expression is a call C with nothing wrapping it (no parentheses, `await`,
`void`, or other operator), whose callee is:

- a **hook**: an identifier named `beforeAll`, `beforeEach`, `afterAll`, or
  `afterEach`; or
- a **test chain**: an identifier named `describe`, `it`, or `test` (the
  root); a property access `X.m`, where `X` is a test chain and `m` is
  `only`, `skip`, `todo`, `concurrent`, `sequential`, `shuffle`, or `fails`;
  or a call `X.m(…)`, where `m` is `each`, `for`, `skipIf`, or `runIf` and
  its arguments, the table or condition, are not part of the chain.

Names are decoded identifier names, compared exactly, and node kinds decide
the rest, so `?.` and type arguments change nothing. Nothing else forms a
chain: an element access, a non-null assertion, parentheses, a receiver such
as `this.it`, another name, or a modifier in the other form (`it.each("n",
fn)`, `it.skip(c)("n", fn)`). A tagged template is not a call, so
``it.each`a | b`("n", fn)`` is not a test call, nor are `it.each(table);`
alone, `test.extend(…)`, `test.describe(…)`, `await test.step(…)`,
`suite(…)`, and `expect(…)`. The modifiers are Vitest 3.2.4's (the
`@vitest/runner` types `ChainableTestAPI`, `ChainableSuiteAPI`, `TestAPI`, and
`SuiteAPI`), so a modifier a later Vitest adds is a known gap. The list is
fixed because fixture objects are often named `test`: `test.runtimeHost.advance(…)`
is not a test. Playwright's `test.describe` and `test.step` are not
recognized. Test calls count wherever their statements are, in helper
functions, conditionals, loops, and unbraced bodies alike, except inside a
computed property name: nothing inside the `[…]` that names a member is
listed, because `--at` gives every position in it to the member
([selection](#labels-keys-and-selectors)).

**Entries.** Each entry has:

- a **position**: the start of its statement's first token, the root or hook
  identifier. No declaration names it and no unnamed site anchors it, so
  `--at` there always selects the test's statement as a
  [statement site](#statement-sites): `--show --at` prints the whole test,
  and `--insert-after --at` inserts after it when the statement is in a
  statement list.
- a **call**: the hook's name, or the root and each modifier in source order
  after a `.`, with `()` after a called modifier (`it.skip`, `it.each()`,
  `describe.concurrent.each()`), spelled with the decoded names that matched,
  so `\u0069t.skip(…)` is `it.skip`.
- for a test chain, a **name**: C's first argument as written, from its
  first token through its last, with every line break (LF, CR, U+2028,
  U+2029) removed and nothing else changed or truncated:
  `"adds %i"`, `` `handles ${"x"}` ``, `Widget.name`. For `.each`, `.for`,
  `.skipIf`, and `.runIf`, C is the outer call, so
  `it.each(table)("adds %i", fn)` is named `"adds %i"`. A callback-first
  call is named by the whole callback, so `it(() => {`, `  expect(1);`, `});`
  over three lines is `1:1 it () => {  expect(1);}`. A hook and a call
  without arguments have no name.

An entry's parent is the nearest test call whose statement contains its
statement. Every other syntax is transparent, including calls that are not
test calls, such as `test.describe(…)`, and an entry with no such ancestor is
at the top level, even inside a helper function. Entries are ordered by
position, which is a pre-order, and every level is shown; there is no
`--depth`.

**Output.** Text prints the file's absolute path, then one line per entry:
two spaces per containment level, `LINE:COLUMN`, the call, and, when the
entry has a name, a space and the name. For this file:

```ts
 1 import { describe, it, test, beforeEach, expect } from "vitest";
 2 class Widget {}
 3 describe(Widget.name, () => {
 4   beforeEach(() => {});
 5   it.each([1, 2])("adds %i", (n) => { expect(n).toBeTruthy(); });
 6   describe.each([["a"], ["b"]])("case %s", (x) => {
 7     it(`handles ${"x"}`, () => {});
 8   });
 9   it.todo("writes later");
10   it.skip("skipped one", () => {});
11   test("with options", { timeout: 1000 }, async () => {});
12   it.skipIf(process.env.CI)("conditional", () => {});
13   test.concurrent("parallel", async () => {});
14 });
15 test.describe("playwright suite", () => {
16   test("pw case", async ({ page }) => {
17     await test.step("first step", async () => {});
18   });
19 });
```

```text
/abs/edge-cases.test.ts
3:1 describe Widget.name
  4:3 beforeEach
  5:3 it.each() "adds %i"
  6:3 describe.each() "case %s"
    7:5 it `handles ${"x"}`
  9:3 it.todo "writes later"
  10:3 it.skip "skipped one"
  11:3 test "with options"
  12:3 it.skipIf() "conditional"
  13:3 test.concurrent "parallel"
16:3 test "pw case"
```

Line 16's `test` is at the top level, since `test.describe(…)` is not a test
call, and `await test.step(…)` is not listed. `--show --at 10:3` prints line
10's whole statement, and `--insert-after --at 10:3` inserts after it
([statement sites](#statement-sites)). The document is
`{"operation":"test-outline","outcome":"ok","file":FILE,"entries":[…]}`, with
the absolute FILE and each entry as
`{"anchor":{"line","column"},"call","name","children":[…]}`: `name` is absent
when the entry has none, and `children`, always present, nests the entries
the text indents.

```json
{"entries":[{"anchor":{"column":1,"line":3},"call":"describe","children":[
  {"anchor":{"column":3,"line":4},"call":"beforeEach","children":[]},
  {"anchor":{"column":3,"line":5},"call":"it.each()","children":[],"name":"\"adds %i\""}, …],"name":"Widget.name"},
 {"anchor":{"column":3,"line":16},"call":"test","children":[],"name":"\"pw case\""}],
 "file":"/abs/edge-cases.test.ts","operation":"test-outline","outcome":"ok"}
```

**Bounds.** `--limit N|all` caps the entries in source order, by default 200
([display limits](#display-limits)); with `-o` and no `--limit`, nothing is
cut. A cut keeps a prefix, so every shown entry's parent is shown and the
document nests the shown entries. The cut text ends with the shared cut line,
and a cut document adds `display`, as an outline's does:

```text
/abs/edge-cases.test.ts
3:1 describe Widget.name
  4:3 beforeEach
  5:3 it.each() "adds %i"
… 8 more entries; see all with: tsparse --test-outline /abs/edge-cases.test.ts --limit all, or write them to a file with -o FILE
```

**Failures.** A file without test calls prints its path alone and exits 0;
its document has `"entries": []`. Input failures are the outline's, through
the same input owner (`navigation::file`), with the same exit codes and
[channels](#exit-codes-and-channels): a file that cannot be read is `Io`
(exit 2), a syntax error is `Syntax` (exit 1), a file nesting deeper than the
parser's grammar budget is `ResourceLimit` (exit 1; 85 nested
`describe("d", () => {` suites list and 86 fail, and other shapes reach the
budget at other counts), and a JSON, JavaScript, or other file is
`Unsupported` (exit 1). With `--json` each is an `error` document naming
`test-outline`. A usage error and a result that cannot be written are each
`CODE: message` (exit 2).

**Library.** `navigation::test_outline_file(path)` returns a `TestOutline`
with the absolute `file` and flat `entries` in source order, each a
`TestEntry` with `anchor`, `call`, `name` (`None` without one), and `depth`, as
`Outline::entries` are; its failures are the outline's `AnalysisError`s. The
owner walks the final syntax tree with an explicit stack and walks a chain in
a loop, so nothing recurses on input.

## Differences from TypeScript 7.0.2 document symbols

TypeScript's `textDocument/documentSymbol` answers an editor-outline question;
this catalog selects exact syntax sites. `tools/navigation-conformance.mjs`
matches entries by selection position and label, then checks every catalog
entry's kind against the syntax at its range in the TypeScript 6.0.3 tree (the
fine vocabulary: `type` is a type alias, not a class; `import`, `export`, and
`variable` are told apart; so are `module` and `namespace`, and `accessor` and
`property`), and every matched entry's compiler kind (through the mapping
below), range, and containment. It accepts only these differences, each counted
in its report and each decided by the entry's own syntax; no allowance passes
to an entry's descendants:

| Code | Difference | Reason |
| --- | --- | --- |
| `shorthand` | No entries for shorthand properties or spreads, which the compiler lists as leaves labelled by the name or the spread expression | `return { run }` must not duplicate the declaration of `run`; a spread declares no name. |
| `transparent` | The declarations inside a spread's expression, or a shorthand property's default, are entries under the enclosing entry; the compiler lists nothing inside the leaf | A spread such as `{ ...{ run() {} } }` or `{ ...build(() => {}) }` contains real methods and functions, which must stay selectable. |
| `merged` | No merging: a later namespace, ambient module, or `declare global` block, or a property assignment, declaration, or signature, parameter property, or accessor (not a shorthand) that shares its name with a sibling class, function, or variable, stays a separate entry with its own children. The compiler merges such a property into every same-named sibling, so it lists the property's contents once under each; the copies are `merged` too | The compiler keeps only the first span; each site must stay selectable with its own excerpt. |
| `untraversed` | No entries from parameters, types, type parameters, decorators, or heritage clauses | They contribute to signatures. The compiler lists them inside initializers and class headers but not for function declarations; one rule is easier to predict. |
| `expando` | No entries for `f.x = …` expando assignments; a function assigned there is an entry labelled by the assigned name where the assignment is | Assignments are not declaration syntax. The compiler moves the assignment's entry under `f`; the declarations inside the assigned function must still match, contained where the assignment is. |
| `jsdoc` | No entries for JSDoc `@typedef` and `@callback` tags | In TypeScript files they declare nothing; the compiler lists them in every file kind (and a comment that mentions `@typedef` mid-sentence becomes one). JavaScript binding is outside this delivery. The compiler entry's range must lie inside a JSDoc comment. |
| `truncated` | Labels are never truncated | The compiler cuts names and callback text at 150 characters plus `...`; exact names are identities here. |

Kinds map to the compiler's `SymbolKind` as follows: `function` 12; `variable`,
`import`, and `export` 13; `class` and `type` 5; `interface` 11; `enum` 10;
`enum_member` 22; `namespace` and `module` 3; `method` 6; `accessor` and
`property` 7; `constructor` 9; and `signature` 6 for `()`, 9 for `new()`, and 7
for `[]`. A matched entry's range must equal the compiler's, except that a
variable binding's range is its statement (or loop declaration list) and an
import or export specifier's is its whole declaration, which must contain the
compiler's narrower range: the compiler's `range` for a variable is its
declaration without `export const`.

After the final review's fixes (2026-10-08), the fixtures and all 5,319
searched Command Center files again match with 0 unexplained differences and
the same totals; keys, which the comparison does not match on, are the only
catalog change. Evidence at plan step 6 (2026-10-08, commit `dfc65e2`) with the pinned 7.0.2
binary and the range and kind checks: the 17 `fixtures/navigation` files
match with 0 unexplained differences (325 compiler entries, 317 catalog
entries, 303 matched; allowances: 17 merged, 7 transparent, 5 untraversed, 5
shorthand, 4 expando, 4 jsdoc), and so does **every** searched TypeScript file
of unchanged Command Center (`7706073d2`): 5,319 files, 696,497 compiler
entries, 668,704 catalog entries, 663,664 matched (allowances: 28,567
shorthand, 3,767 untraversed, 3,221 merged, 2,647 transparent, 424 expando, 84
truncated), as does a 69-file sample that adds eight dependency declaration
files, among them `@tiptap/core`'s, which repeats `declare module
'@tiptap/core'` 57 times ([validation](validation.md#navigation)). On
2026-10-07, two Command Center samples at
revision `7706073` also matched with 0 unexplained differences: a 42-file
sample (the
largest tests and sources, the largest TSX files, and a spread of smaller
files) with 38,535 compiler entries and 36,709 catalog entries, 36,427 matched
and their ranges checked (allowances: 1,868 shorthand, 232 untraversed, 192
transparent, 123 merged, 8 expando, 6 truncated); and a 43-file sample of other
large and sampled sources and TSX files plus eight declaration files
(`@types/node` `fs`, `child_process`, and `http`, `@types/react`,
`@types/better-sqlite3`, `@types/d3-array`, `next/types/global.d.ts`, and
generated Next.js route types) with 29,833 compiler entries and 28,601 catalog
entries, 28,386 matched (allowances: 1,329 shorthand, 131 merged, 108
untraversed, 95 transparent, 14 truncated, 10 expando).

## Verification

```sh
export TYPE_SCRIPT_NATIVE=/path/to/typescript-<platform>-7.0.2/package/lib/tsc
export TYPE_SCRIPT_API=/path/to/typescript-6.0.3/package/lib/typescript.js
node tools/navigation-oracle.mjs --pretty fixtures/navigation/factory.ts
node --test tools/navigation-oracle.test.mjs tools/navigation-conformance.test.mjs
cargo build --offline
node tools/navigation-conformance.mjs fixtures/navigation [PATH ...]
```

`tools/navigation-oracle.mjs` speaks the language-server protocol to
`tsc --lsp --stdio` and requires `Version 7.0.2`. It sends file text without a
leading byte order mark, as editors and the compiler's own reads do, and
converts the protocol's positions, whose lines end only at CR, LF, and CRLF
(7.0.2's `ComputeLSPLineStarts`), to ECMAScript lines, which also end at U+2028
and U+2029, as `tsc` diagnostics and this tool count them. It owns
the server process: a failed start ends and reaps it before rejecting, and
closing reports a server that exits with a nonzero status or a signal. After
`shutdown` it ends the server's input rather than sending `exit`, because
7.0.2's `Server.Run` exits with status 1 whenever it handles `exit` before it
reads the end of its input. Its harness test pins each fixture's document
symbols in `fixtures/navigation/<file>.symbols.json`, so a compiler change is
detected; regenerate them with `--write-expectations`.

The conformance runner uses `TSPARSE` (default `target/debug/tsparse`), writes
`.cache/navigation-conformance.json`, and exits 1 on any unexplained
difference. The registered `semantic` validation command runs it over
`fixtures/navigation` (see [validation](validation.md#navigation)).
`tools/navigation-conformance.test.mjs` proves the comparison fails on
regressions: with real TypeScript 6.0.3 syntax, it mutates the catalog entries
pinned in `fixtures/navigation/<file>.outline.json` (written by
`node tools/navigation-conformance.mjs --write-outlines FILE` and kept equal to
the tool's output by `tests/navigation_cli.rs`) by dropping an expando's
assigned function or its contents, reparenting a declaration under an expando,
listing the returned `{ startEligible }` shorthand, and giving an entry a wrong
range or fine kind. `tests/navigation.rs` and `tests/navigation_cli.rs` assert
the catalog and command contracts with expectations taken from compiler
observations: document symbols, workspace symbols for which imports and
exports have keys, and `tsc` diagnostic positions for the byte order mark. On
a real project, `tools/navigation-flows.mjs` repeats the outline, show, and
ambiguity round trips through the service and with `--direct`, and
`tools/name-selection.mjs` measures how many exported declarations a
project-wide name selects ([validation](validation.md#command-center-checkpoint)).

# Declaration navigation

`tsparse --outline` lists the declarations of one TypeScript file, and
`tsparse --show` prints the exact source of one of them. Without `--project`
they read only the named file; with `--project TSCONFIG` they answer from the
project's captured snapshot, over the files reference queries search, through
the same local service as references. The Rust library exposes the same
operations as `typescript_parser::navigation::outline_file` and `show_file`,
and over a project's searched files as `Project::outline` and `Project::show`
(see [Library](#library)). The design is recorded in
[navigation-design.md](navigation-design.md); this document describes the
implemented behavior and where it deliberately differs from TypeScript 7.0.2.

## Commands

```sh
tsparse --outline src/runs/host.ts
tsparse --outline --depth all --json --pretty src/runs/host.ts
tsparse --show --name createRunHost.startEligible src/runs/host.ts
tsparse --show --at 42:10 src/runs/host.ts --json -o shown.json
tsparse --outline --project tsconfig.json src/runs/host.ts
tsparse --show --project tsconfig.json --name createRunHost.startEligible
tsparse --show --project tsconfig.json --at 42:10 src/runs/host.ts --json
tsparse --show --project tsconfig.json --at /abs/src/runs/host.ts:42:10
tsparse --show --project tsconfig.json --direct --name startEligible src/runs/host.ts
```

| Operation | Project | File | Selector | Options |
| --- | --- | --- | --- | --- |
| `--outline` | optional | exactly one named file | none | `--depth N` or `--depth all` (default 2), `--json`, `--pretty` with `--json`, `-o` |
| `--show` by name | optional | required without `--project`; optional with it, to narrow the search | `--name NAME` | `--json`, `--pretty` with `--json`, `-o` |
| `--show` by position | optional | exactly one named file, as the operand or in `--at` | `--at LINE:COLUMN FILE` or `--at FILE:LINE:COLUMN` | `--json`, `--pretty` with `--json`, `-o` |
| `--references` | required | required for `--at`, as for show; optional for `--name` | exactly one of `--name` (one identifier), `--at` | `--pretty`, `-o` ([find references](references.md)) |

`--direct` answers a project query (outline, show, or references with
`--project`) in the CLI process instead of the [service](#project-mode);
`--references-direct` remains the reference spelling. One request parser
decides validity before anything runs, and nothing starts or loads for an
invalid request. Navigation rejects, as usage errors: stdin (`-`), `--direct`
without `--project`, `--check`, `--tsx`, `--pretty` without `--json`, `--depth`
with `--show`, `--at` or `--name` with `--outline`, both or neither of `--at`
and `--name` with `--show`, a missing file, a FILE operand beside
`--at FILE:LINE:COLUMN`, more than one file, a value
option (`--project`, `--name`, `--at`, `--depth`, `-o`) given twice, a second
operation such as `--references` or a service action, and `--json` or
`--depth` outside navigation. Parse and reference requests keep their
established rules, including the last value of a repeated option. Project
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
Contents that are not UTF-8, including UTF-16, are unreadable (`Io`). Parse
mode is unchanged: it parses the bytes it is given, mark included.

### Project mode

With `--project`, a file must be one of the project's searched files, the files
reference queries search, named as they name them: relative to the working
directory, matched lexically under the project's case policy. Dependency and
standard-library files and files outside the configuration's inputs are
`OutsideProject`; a searched JSON module is `Unsupported`. `--show --name`
without a file searches every searched file, and candidates across files are
ordered by file and then position. The whole project loads as for references:
a project that fails to load answers no operation, and its failure is never
replaced by an answer from the file alone.

On macOS and Linux, project queries go to the project's service, the one that
answers reference queries for the same configuration (its `--project` path
made absolute against the working directory, without resolving `..` or
symbolic links), started on demand and stopped with `--references-stop`;
other platforms answer directly. The
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
service answers alike:

| Outcome | Exit | Text | JSON (`--json`) |
| --- | --- | --- | --- |
| Outline, or one declaration found | 0 | Result on stdout or `-o` | `"outcome": "ok"` / `"found"` document on stdout or `-o` |
| Ambiguous or missing selection | 1 | Diagnostic and candidate anchors on stderr | `"ambiguous"` / `"not_found"` document on stdout or `-o` |
| Syntax error, unsupported input, project analysis failure (`Config`, `Binding`, `UnresolvedImport`) | 1 | `path:line:column: message` on stderr | `"error"` document on stdout or `-o` |
| Unreadable file, invalid position, file outside the project | 2 | Diagnostic on stderr | `"error"` document on stdout or `-o` |
| Usage error, service transport failure, failure to write the result | 2 | Diagnostic on stderr | Diagnostic on stderr |

A JSON outcome never writes stderr, so its document and exit status identify
it. A document is built only in the format requested. Its `file` and
`error.path` hold exact paths only: a path that is not Unicode (possible on
Linux, for example under a working directory whose name is not Unicode) is
`null` there, and the error message names it in a lossy form. A text failure
never creates or replaces the `-o` file. A transport failure (the service
could not start or be reached, or its runtime directory is not private)
leaves no outcome, so it is a diagnostic in every format, like a usage error;
it never becomes a direct load. A result that cannot be written is one
diagnostic, never followed by a second document. An empty file has an empty,
successful outline. Unlike reference name queries, which succeed with no
groups, a show selector that matches nothing exits 1. A position outside the
file, past the end of its line, or inside a surrogate pair is invalid (exit 2);
a valid position on no declaration name or anchor is a missing selection.

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
property is still a `property`.

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
label, and anchor, and `--at` on its name shows its whole declaration.
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
dotted namespace, a constructor's `constructor` keyword) or anchor contains
the position; a name's end is outside it. An entry's **anchor** is its name's
first character, the first segment of a dotted namespace, or the first token of
an unnamed site (including decorators and modifiers). Outline entries and
ambiguity candidates always carry an anchor that `--at` selects.

When several entries match, show lists every candidate rather than choosing:
overloads, same-named locals in different blocks, merged declarations, and a
renaming import or export beside a declaration of the same name all remain
separate.

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
{"operation":"show","outcome":"not_found","file":"/abs/src/host.ts","query":{"at":{"line":3,"column":5}}}
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
 "message":"not one of the project's searched files","path":"outside/x.ts","position":null}}
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
its containment `depth`; display depth is the caller's choice. Failures are
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

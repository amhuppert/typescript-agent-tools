# Find references

`tsparse --references` groups the declarations and occurrences of a symbol in
one configured TypeScript project. The Rust library exposes the same operation
as `typescript_parser::project::Project::find_references`, with query and result
types in `typescript_parser::references`. The design is recorded in
[find-references-design.md](find-references-design.md); this document describes
the implemented behavior and where it deliberately differs from TypeScript.

## Command

```sh
tsparse --references --project ./tsconfig.json --at 12:8 src/app.ts
tsparse --references --project ./tsconfig.json --at src/app.ts:12:8
tsparse --references --project ./tsconfig.json --name run
tsparse --references --project ./tsconfig.json --name run src/app.ts --pretty
tsparse --references --project ./tsconfig.json --name run --calls --no-tests
```

`--project` names a configuration file; no ancestor directory is searched.
Exactly one of `--at LINE:COLUMN` (one-based line, one-based UTF-16 column, with
a required FILE) or `--name NAME` (with an optional FILE that narrows the
candidates) selects the symbols. `--at FILE:LINE:COLUMN`, the form navigation
text prints, names the file without a FILE operand; it is read from the right,
so FILE may contain `:`, and giving both is a usage error. A bare NAME selects
lexical declarations and aliases. A dotted NAME such as `RunHost.observe`
selects by declaration containment, with explicit not-found/ambiguous results;
`--at` also selects supported semantic member tokens. Argument paths are relative to
the working directory. `--pretty` and `-o OUTPUT` keep their parse-mode meanings; stdin,
`--check`, `--tsx`, and `--depth` are rejected. `--direct` (or
`--references-direct`) answers in the CLI process instead of the service.
A result goes to stdout or `-o`; by default a failure is a diagnostic on stderr
and never creates or replaces the `-o` file. With `--json`, an analysis failure
is delivered as a document to stdout or `-o`. Usage, transport and delivery
failures remain stderr diagnostics, under the delivery policy navigation shares
([exit codes and channels](navigation.md#exit-codes-and-channels)).

`--calls` keeps only direct call, optional call, constructor and tagged-template
targets. Parentheses, non-null assertions, type assertions, `as`, `satisfies`
and instantiation wrappers are peeled. Namespace accesses count their member,
not their receiver. Taking a function as a value, passing it as an argument,
or calling its `.call`, `.apply` or `.bind` member does not count as a call to
that function. Implicit decorator calls and JSX tags are excluded; explicit
`@factory()` calls are included. Declaration metadata survives an empty filter.

`--no-tests` removes occurrences in exact case-sensitive directory segments
`test`, `tests`, `__tests__`, or basenames ending `.test` or `.spec` followed by
`.ts`, `.tsx`, `.mts` or `.cts`, on normalized project-relative `/` paths.
Selection, loading, dependencies and declaration metadata still include those
files. `fixtures`, `spec`, `Tests`, and `.TEST.ts` are retained. Both filters
are echoed in the result and rejected on service-administration commands.
Results identify their coverage as `lexical` or `member`. Bare-name lexical
results list matching catalog members in `unsearchedMembers`. Member searches
report `complete` or `partial` coverage and located gaps; they do not merge
unrelated properties by spelling. See [types.md](types.md) for semantic selection,
partial results, and process limits. Final delivery validation is tracked in
[the completion checkpoint](types-completion-correctness-2026-10-13.md).

| Exit | Meaning |
| --- | --- |
| 0 | A result, including `groups: []` for a token-free position or an unmatched name. |
| 1 | Invalid configuration content or inheritance, a syntax error, unsupported required input, invalid binding, a name that resolution looks up and does not find (an import, a `types` entry, or a `/// <reference>` target), or an unsupported or unresolved selected target. |
| 2 | Invalid flags or position; a file the invocation or configuration names to be read, the `--project` file, an `extends` target, or a `files` entry, that does not exist; any file or directory that exists but cannot be read; or a query file that is not one of the project's searched files (including a missing file). |

Diagnostics on stderr have the form `path:line:column: message` when a location
is known. Sources, configurations, and package manifests are read as TypeScript
reads them (`decodeBytes`): a leading UTF-8 byte order mark is not part of the
text, so first-line columns in queries, results, and diagnostics are the ones
`tsc` reports.

## Persistent CLI service

On macOS and Linux, ordinary CLI project queries use a local service: reference and type
queries, and [outline and show](navigation.md#project-mode) with `--project`.
One service per absolute configuration spelling answers these through one
session request boundary, which checks the captured filesystem inputs before
every request and loads a new generation after any change; there is no second
service, cache, or refresh path per operation. It reuses a complete snapshot
while its inputs match. The check rereads and compares every captured input,
so it dominates a warm request on a large project: on Command Center (7,639
inputs) a warm query takes about 0.4 s, nearly all of it the check, and a
rebuild after an edit about 10 s ([measurements](performance-command-center.md#current-state-2026-10-08)).
`--references-status --project CONFIG` reports `running`, `pid`, and `generation`
(the number of successfully published snapshots); status does not check freshness.
`--references-stop --project CONFIG` stops it, and `--direct` (or
`--references-direct`) performs a fresh load in the CLI process through the
same request boundary, giving the same output and exit code.
`--references-serve --project CONFIG` runs the service in the foreground for a
supervisor. Background services use null standard streams and a separate process
group, close every other descriptor they inherit, and expire after ten idle
minutes.

The private runtime directory defaults to `/tmp/tsparse-UID`. Override it with
`TSPARSE_SERVICE_DIR`, naming a directory owned by the current user with mode 0700;
its parent must exist. Keep paths short enough for Unix sockets: a socket
address holds about 100 bytes (104 on macOS, 108 on Linux), and the socket's
name adds 22 to the directory. A longer path fails every service request with
an exit-2 diagnostic that names the socket and asks for a shorter
`TSPARSE_SERVICE_DIR`. Set
`TSPARSE_SERVICE_IDLE_SECS` to a positive integer to change the idle timeout.
Configuration spellings and compiled build identities have separate services.
Old-build services expire normally. Lock files remain after shutdown so concurrent
launchers always lock the same inode; do not remove them while services run.

Requests are limited to 1 MiB including their terminating newline. Socket reads
and writes at the server allow five seconds without progress; clients wait up to
five minutes for a result. A transport or runtime-directory failure is exit 2,
a stderr diagnostic even for navigation's `--json`, and never a silent direct
load. Before either a direct or a service answer, the client makes a query's
file absolute against its own working directory
(`ProjectRequest::make_absolute`, for every operation), so both answer alike
from any directory; diagnostics about that file (`OutsideProject`,
`InvalidPosition`) keep the caller's spelling, and a working directory that
cannot be read is an `Io` failure naming the file as spelled.
Other platforms use direct loading and reject service administration flags.
See [the design](reference-service-design.md) for ownership and consistency limits.

## Library

`ProjectSession::new(config)` creates a lazy refreshable session.
`session.execute(ProjectRequest::References(query))` validates or rebuilds and
returns either `ProjectResponse::References` with the usual result or an
`analysis::AnalysisError` (`AnalysisError::Input` / `AnalysisError::Query`).
The session has this one
request method for every operation (`References`, `Outline`, `Show`), so each
one refreshes the same way; the CLI's direct queries and the service both
answer through it. Configuration
paths are fixed at construction; relative query paths use the caller's current
directory. `session.generation()` counts successful publications. Failed refreshes
discard the old generation. An unreadable *previous* input forces a full load:
current selection decides whether that input is still required. Required input
failures are reported by the loader, never answered from stale state.

`Project::references(ReferenceRequest { query, filters })` and
`ProjectRequest::ReferenceSearch(request)` apply the reporting filters at the
shared lexical/semantic selection owner. `find_references` keeps its existing name/position
selection with default filters; both return the additive coverage and advisory
fields below. A legacy library name containing `.` keeps its exact lexical
question and does not gain containment selection.

`Project` retains its immutable, explicitly captured snapshot semantics:

```rust
use std::path::Path;
use typescript_parser::project::Project;
use typescript_parser::references::ReferenceQuery;
use typescript_parser::source::SourcePosition;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project = Project::load(Path::new("tsconfig.json"))?;
    let result = project.find_references(ReferenceQuery::At {
        file: "src/app.ts".into(),
        position: SourcePosition { line: 12, column: 8 },
    })?;
    for group in &result.groups {
        println!("{} occurrences", group.occurrences.len());
    }
    Ok(())
}
```

`load` reads each program input once and returns either a complete immutable
index or an `analysis::InputError` (`Io`, `Config`, `Syntax`, `ResourceLimit`
for a source nesting deeper than the parser's grammar budget, `Unsupported`,
`Binding`, or `UnresolvedImport`) with a path and position where available. Only a path
that does not exist, or that runs through a file, is absent. Any other failure
to inspect or read a file or directory, such as a package manifest, a type
root, or a directory entry, is an `Io` error. Queries read
no files: a query path is made absolute against the working directory and
matched lexically against each searched file's canonical path, the paths
through which loading reached it, and, for a file under the configuration
directory, its path under that directory as `--project` spelled it (such as a
symlink). A query path names a file only by its exact text: one that is not
Unicode, which a lossy conversion could turn into another file's name, is
`AnalysisError::Input` with kind `Unsupported`, never a file, for every
operation. Results reflect the captured contents even if files change
afterwards. Queries fail with `AnalysisError::Query` (`OutsideProject`,
including an empty path, `InvalidPosition`, `UnsupportedTarget`, or
`UnresolvedTarget`), or with `AnalysisError::Input` for an unrepresentable
path or a working directory that cannot be read (`Io`). A `QueryError`'s
`path` is the file as the query spelled it for `OutsideProject` and
`InvalidPosition`, and otherwise the selected token's file as a canonical
absolute path, whichever entry point selected it. Both error kinds serialize
as their variant names. Loading and querying share
normalization and the captured filesystem case policy; parent traversal is
clamped at an absolute path's root. Results own their data and
can outlive the project.

## Result

```json
{
  "operation": "references",
  "outcome": "complete",
  "project": "/abs/path/tsconfig.json",
  "groups": [{
    "declarations": [{ "name": "run", "location": { "file": "src/api.ts", "start": { "line": 1, "column": 17 }, "end": { "line": 1, "column": 20 } } }],
    "occurrences": [{ "name": "run", "location": { "...": "..." }, "roles": ["declaration", "export"] }]
  }],
  "filters": { "calls": false, "excludeTests": false },
  "coverage": { "baseline": "lexical", "status": "complete", "gaps": [] },
  "unsearchedMembers": { "count": 0, "selectors": [] }
}
```

Each group is one underlying symbol. `declarations` lists all of its
declarations, including merged declarations and declarations in installed
packages and standard libraries. `occurrences` lists the symbol's tokens in the searched files: its
declarations (`declaration`), references (`use`), and names in import and
export clauses (`import`, `export`). A token can carry several roles, such as
`export function run` (declaration and export). A token that denotes more than
one symbol, such as `export { Shape }` naming both an interface and a constant,
appears in each group.

File paths use `/` and are relative to the configuration's canonical
directory, with `../` where needed; `project` names the configuration in that
directory, so joining the two locates the file even when `--project` is
spelled through a symlink. Ranges are exact identifier tokens without trivia; `end` is
exclusive. Occurrences are ordered by file and position, groups by their
earliest declaration.

`complete` describes the existing lexical question and the effective filters.
It does not establish receiver/member identity, infer types, or certify all
TypeScript semantic references. Lexical queries produce no semantic coverage gaps and do not demand receiver or
member types. The shared request boundary still performs checker initialization.

For a bare name, `unsearchedMembers.selectors` lists matching method, accessor,
property and enum-member declarations from the captured Q2 catalog. FILE narrows
these declarations; `--calls` and `--no-tests` do not hide them. A position query
has an empty advisory. Each suggestion has `file`, `selector`, and
`anchor: {file,line,column}`. The key match is exact decoded text, including
empty and normalized numeric keys; it is distinct from selector containment.
Computed/unkeyed sites and inferred members absent from Q2 are not enumerated.

Suggestions are ordered by exact file, selector (null before strings), then
anchor. Entries with one file/selector share the earliest Q2 anchor, including
overload, accessor, merged and static/instance display duplicates. An
unspellable key or ancestor keeps `selector:null` and is deduplicated by
file/anchor instead; distinct anchors remain visible. `count` equals the emitted
suggestions. These are catalog suggestions, not a semantic identity merge.
Their anchors replay `--show --at`. In particular, a parameter property's Q2
name anchor still selects its lexical parameter for reference queries;
its first property-producing modifier is a semantic member anchor.

Declarations that have no identifier use an anchor: an anonymous default export
(`export default function () {}`, `export default 42`) is named `default` at the
`default` keyword, `export = expression` is named `export=` at its `=`, a module file used as a namespace (`import * as ns`, `import x =
require()`, `export * as ns`) is named by its path at line 1, column 1, and an
ambient module (`declare module "x"`) is named `x` at its string literal.

## Lexical selection

This section describes the lexical projection. Member declarations and uses
are handled by the semantic selection owner described in [types.md](types.md),
including dotted selectors and parameter-property modifier anchors.


A position must fall inside an identifier token, start inclusive and end
exclusive. Whitespace, punctuation, and keywords give an empty result. Member,
property, enum member, label, JSX attribute, intrinsic element, tuple label,
import attribute, `import.meta`, `new.target`, `arguments`, `undefined`,
`globalThis`, and `this` in a type query are `UnsupportedTarget`. A name with no declaration among the
program inputs is `UnresolvedTarget`.

A name query is exact and case-sensitive on decoded identifier text, so
`\u0061bc` and `abc` are the same name. Candidates are declaration names, local
import names, and exported alias names in the searched files; FILE restricts
candidates, not results. Every matching candidate's symbols are grouped, so two
shadowed declarations give two groups and two aliases of one symbol give one.
A candidate that is not a supported target, such as an import of a member or an
export of `globalThis`, makes the query fail with `UnsupportedTarget`, and an
unresolved candidate with `UnresolvedTarget`, exactly as a position query on
that token would, rather than silently omitting it.

## Binding model

Each scope maps a name to at most one symbol per meaning (value, type, and
namespace) plus an optional import or export alias. A reference asks for the
meaning its syntax requires and searches outward, so a type alias and a constant
with the same spelling remain separate groups and a type does not shadow a
value. Declarations that TypeScript merges become one symbol: function
overloads, interfaces, class and interface, namespace with namespace, class,
function, or enum, enums, and redeclared `var` and parameters. A namespace is
also a value only when TypeScript counts it as instantiated: when its body
holds anything other than interfaces, type aliases, uninstantiated namespaces,
non-exported imports, and `export { ... }` of such declarations. A re-exported
name declared by an `import =`, or by no enclosing block, counts as a value. So
a namespace of types shares its name with a value instead of conflicting with
it. A module object, which a namespace import, `import = require`, or `export
as namespace` names, is a value and a namespace; an `import =` alias of an
entity name takes every meaning its target has.
Conflicting declarations, such as two `let x`, fail with a `Binding` error
where the error is reported. One decision says whether a diagnostic at a
place is reported, for every binding error and every missing input (an
unresolved module, reference directive target, or augmented module): it is
reported only in a project file, and only where TypeScript reports it there.
TypeScript reports nothing in a file it skips (`SkipTypeChecking`): every
file under `noCheck`, a declaration file under `skipLibCheck`, a standard
library under `skipDefaultLibCheck`, a JSON file, and a file whose last
`// @ts-check` or `// @ts-nocheck` leading comment is `@ts-nocheck`; nor on a
line that a `@ts-ignore` or `@ts-expect-error` comment suppresses (below). A
dependency or standard library file is never reported here, whatever
TypeScript would report: its declarations bind as they allow, so conflicting
declarations in one scope merge, and a name that depends on a missing input
is unresolved. Where TypeScript reports one diagnostic at several
declarations, such as a duplicate declaration at each, any reported one fails
the load; where none is reported, a conflict in one scope merges, and across
files TypeScript's choice stands (below). Libraries come first, so a project
declaration that conflicts with a library global, such as `let Array` in a
script, fails where the project declares it. Explicitly selected roots are
project files even inside `node_modules`. This decision is separate from
whether a file's occurrences appear in results.

Scopes follow TypeScript: function-scoped `var` and block-scoped `let`, `const`,
`using`, classes, and strict-mode functions; separate parameter and body scopes,
so a parameter default cannot see body declarations; class and interface type
parameters shared by merged declarations; mapped type parameters; and `infer`
declarations visible only in their conditional type's true branch. Namespace
blocks see each other's exports but not each other's locals. Script files share
the global scope with `declare global` blocks and UMD globals. Decorators
resolve where their class is declared.

The global table is built as TypeScript's `initializeChecker` builds it, in
program order. Each script's top-level declarations, bound together by the
binder's rules, merge into it file by file as the checker merges symbols
(`mergeGlobalSymbol`); after each file, its UMD globals (`export as
namespace`) join only where no global has their name yet, so a script
namespace declared before a UMD file keeps the name and the UMD global is
dropped. A UMD global is declared only at the top level of a module that is a
declaration file; elsewhere TypeScript declares nothing and reports TS1316
(not at the top level), TS1314 (in a script), or TS1315 (not a declaration
file), a `Binding` error where reported. Then each file's `declare global`
blocks merge into it by the same merge as module augmentations (below): a
declaration meeting an alias, such as a UMD global or an `export import`
inside a namespace, merges with what the alias names. Last, as TypeScript
defers them, ambient modules declared in more than one script merge: each
later file's declarations of the name, bound together, merge into the module
of that name in program order by the same merge, so an `export import` alias
in one file and an interface in a later one merge into what the alias names,
while the alias after the interface conflicts with it (TS2440), as within one
file. The module's first declaration in that order decides whether it is
shorthand: after `declare module "x";`, every name imported from `"x"`
denotes the module, even a later file's export. Pattern ambient modules
declared in several files merge the same way. Excluded pairs across files,
such as `let x` in two scripts, are a `Binding` error where TypeScript
reports TS2451 or TS2300 at a reported declaration of either side; where
neither side is reported, TypeScript's choice stands and the earlier
declaration keeps the name.

Aliases are directed edges. An import, re-export, `export =`, `import =`
alias, `export as namespace` global, or `export * as` name resolves to the
underlying symbol, following star re-exports (local exports shadow them, and
different targets for one name are ambiguous) and `export =` targets. A
type-only alias does not supply a value outside type positions, so
`import type { C }` followed by `new C()` leaves `C` unresolved. Restrictions
belong to each resolved meaning: a same-named local value beside an imported
type remains usable through ordinary re-exports, while an explicit type-only
re-export restricts that value too. `ns.name` and
`N.Type` resolve through module and namespace exports; other property accesses
are not reference targets. Ordinary value flow, such as `const copy = run`,
creates no alias.

A `declare module "m"` block is an augmentation where TypeScript's
`IsModuleAugmentationExternal` says so: at the top level of a module, or, with
a non-relative name, directly inside an ambient module, or a `declare global`
block, at the top level of a script. At the top level of a script it is an
ambient module, and a relative name there is a `Binding` error where
reported (TS2436). Anywhere else it is a `Binding` error where reported
(TS2435) and merges nowhere, so in a dependency it does not shadow a package;
a relative name directly inside a script's ambient module or `declare global`
block merges nowhere either, without an error. As TypeScript's parser lists
modules (`collectModuleReferences`), a string-named module counts only in an
ambient context: declared, inside a declared module, or in a declaration
file. Otherwise it is a `Binding` error where reported (TS1035) and is no
augmentation, though at the top level of a script it is still an ambient
module. The same placement rule applies to `declare global`: it merges into
the global table only at the top level of a module or directly inside a
script's top-level ambient module; elsewhere it is a `Binding` error where
reported (TS2669), and otherwise its names stay local to it.

A file's augmentations of one module name form one symbol whose declarations
bind into its own table, as in TypeScript's binder. Once every other
declaration is bound, each merges into what it augments, file by file in
program order, as TypeScript's `mergeModuleAugmentation` and `mergeSymbol` do:
one merge, applied recursively to the namespaces it declares and to their
exports. Each declaration merges with the module's export of its name: a
declared export; the target of an export alias (`export { X }`,
`export type { X }`, `export { Y as X }`, `export { X } from`, or an exported
import, also inside a namespace), which then becomes the module's own export
without the alias's type-only restriction; or the symbol a non-ambiguous
`export *` supplies. The original declaration, every augmentation, the export,
and its consumers form one group; merged interfaces and classes share their
type parameters, and a meaning the merge adds, such as a class's value, is
visible wherever the original declaration is. An export of a shorthand ambient
module names the module itself, so a declaration merged through it gives the
module that meaning: every import of the module and its module object then
have it. An export that names a JSON module's value or property merges as the
property TypeScript resolves it to: a declaration that excludes a value, such
as a `const` or an instantiated namespace, conflicts with it (TS2451, TS2300),
while an interface or a namespace of types merges beside it, so the name keeps
denoting the value in an expression. Excluded pairs, such as a type alias
augmenting an interface, are a `Binding` error where TypeScript reports them
at a reported declaration of either side; where neither is reported,
TypeScript's choice stands: the augmentation replaces an export alias it
conflicts with, and the original declaration stays where the augmentation
conflicts with the declaration itself. A module whose `export =` names a
namespace, module, or enum is augmented through that target; any other target
(TS2671), or, when the file's augmentations of that module declare values, a
target that also declares a variable (TS2649) or is a const enum (TS2567), is
a `Binding` error where reported, and otherwise binds nothing. As in
TypeScript, an augmentation's name adds no file to the program: its target is
a program input only when another route includes it. Augmenting a module that
is not in the program (TS2664) or a script (TS2306) is an `UnresolvedImport`
error, decided once for both: not where the augmentation's parent is ambient,
in a declaration file or inside a script's ambient module, nor where the error
is not reported, as for a missing import. There the augmentation binds
nothing.

An augmentation of a specifier that only a pattern ambient module (`"*.css"`)
matches extends that specifier alone, one way: it has the pattern's and the
augmentation's declarations, while other matching specifiers, and the
pattern's own declarations, keep only the pattern's. The specifier gets a
module of its own at that augmentation's turn, as TypeScript's
`mergeModuleAugmentation` makes it: a merge before it, such as one through
another module's `export * as X from "x.css"`, merged into the pattern itself,
so every matching specifier has what it added. And once any merge has merged
into the pattern module, including a pattern declared in several files, an
augmentation of a matching specifier merges into the pattern both ways, as
into any module, because TypeScript's resolved module is then no longer the
pattern's own. The specifier's module is shorthand when the pattern is and
the augmentation declares no values. While augmentations merge, the
specifier's module already exports everything the pattern does, so an alias
resolved meanwhile, such as another module's `export { X } from "x.css"`,
finds the pattern's declaration. An alias the pattern declares
beside the augmentation's declaration stays an alias declaration of the
merged name, as in TypeScript: its group lists what the alias names, but no
compatibility check compares that declaration, and its type parameters stay
its own. The pattern's own type parameters stay its own too, so the merged
interface has the augmentation's parameters followed by the pattern's, and
each declaration must list a prefix of them that includes every parameter
without a default (TS2428): `interface X<T>` in both is a `Binding` error.
That check, as every merge check, reads the declarations of the symbol a
checked declaration denotes, so a project pattern that a dependency's
augmentation merges into one way is not checked against it.

Aliases and qualified references resolve during loading in dependency order,
with an explicit task stack. As in TypeScript, a name with local declarations
resolves to them without following a same-named import for meanings they lack,
so an alias cycle that returns through such a name resolves to its locals;
the import then conflicts with the local declaration (TS2440), a `Binding`
error where reported. Every alias in a cycle of aliases alone (TS2303) is
unresolved rather than an error. Either way the result does not depend on which file
resolution reaches first.

## Project configuration

`tsconfig.json` and JSON modules are read as TypeScript reads both, with one
JSON reader (`parseJSONText`): its scanner's comments, ending at any ECMAScript
line terminator, trailing commas, a leading `#!` line, its numbers (`0x10`,
`1_000`, `.5`, and a leading `-`, but not `01`, `08`, or `1n`), and
JavaScript string escapes such as `\x41` and `\u{41}` (but not `\01` or
`\8`); keys and string values must be double-quoted, and any other value or
member, such as `+1`, `NaN`, or a method, is a syntax error. A string with an
unpaired UTF-16 surrogate keeps it, so it equals no other name; one that must
name a file, module, option value, or `paths` pattern is `Unsupported`. A
configuration's text may be empty or only comments, which is an empty
configuration; any other root must be an object, or an array holding one,
which TypeScript reads in its place. A `?` after a key is an error in a
configuration (TS8009), though not in a JSON module. A repeated key keeps its
first position and its last value, and a property with an empty name is left
out, so `"paths": { "": ... }` declares no pattern; a JSON module keeps such a
key. Every occurrence of an option this tool reads is validated, as TypeScript
validates each one it converts: a repeated key, each `compilerOptions` object
of a file, and each file of the `extends` chain, so an invalid value that a
later one overrides is still a `Config` error. Within a file, an option takes
the last occurrence that sets it, across all its `compilerOptions` objects: a
`null`, or an empty string for an enumeration such as `module`, sets nothing
and leaves an earlier value, while a list's `null` clears it. A file with
`extends`, even an empty list, takes each base's options in order and then
its own; a setting that leaves the option unset, such as a list's `null` or an
empty `jsxImportSource`, does not replace an inherited value, but a `null` as
the last value in the file's last `compilerOptions` object unsets the option,
as a base's does over the bases before it. `extends` itself may not be
`null`. `package.json` is read as strict
JSON, as TypeScript 7 reads it: a manifest with a comment, trailing comma,
single quote, other escape, raw control character, non-ASCII whitespace, or
unpaired surrogate is unparsable, so its fields are ignored while it still
marks a package scope. A configuration, manifest, or JSON module nested more
than 128 arrays and objects deep is `Unsupported`: that is this tool's limit,
not an unparsable file. `extends` accepts a
string or an ordered array of relative, rooted, or package configurations
(through `node_modules`, a manifest `tsconfig` field, `exports`, or a package's
`tsconfig.json`), with `${configDir}` and inheritance cycles reported as
configuration errors. Path settings stay relative to the configuration that
declared them. `files`, `include`, and `exclude` replace inherited values: a
configuration that has the key at all, even as `null`, inherits none, and a
`null` then selects as if the key were absent; otherwise the last base whose
own value, after its own inheritance, is a list supplies it.

Roots follow TypeScript's selection: `files`, `include` and `exclude` globs,
`foo.ts` hiding `foo.d.ts`, and default excludes of `outDir` and
`declarationDir`. Program inputs add every resolved import, re-export,
`import = require`, import type, dynamic `import("literal")`, module
augmentation, `/// <reference path>`, `/// <reference types>`, and `types`
entry, JSON files included, plus the automatic JSX runtime of each `.tsx` file under `jsx`
`react-jsx` or `react-jsxdev`, from `jsxImportSource` or a block-comment
`@jsxImportSource` pragma (`@jsxRuntime classic` turns it off). Searched files are the roots and their local dependencies; files reached
through packages or type packages are program inputs whose declarations
resolve names but whose own occurrences are not reported.

Program order, which orders the global table's merges, pattern ambient
modules, and module augmentations, is TypeScript's (`collectFiles`, as
`tsc --listFilesOnly` prints it): starting from the roots, the root
libraries, and the `types` packages, each file follows the files its
subtasks reach, depth first, and libraries come first. A file's subtasks are,
in order, its `/// <reference path>`s, its `types` and `lib` references, the
automatic JSX runtime, its imports and re-exports, then its dynamic imports
and import types; within each group they follow source order, wherever they
appear (an `else` branch, a call's arguments, a type annotation), as
TypeScript finds them in the text. A module augmentation's name adds no file.
On unchanged Command Center, the 7,639 inputs this selects, their order, and
their provenance (library, dependency, or searched) are the pinned compiler's
(`tools/program-inputs.mjs`, see [validation](validation.md#command-center-checkpoint)).

Standard libraries follow TypeScript 7.0.2. A project with root files and
without `noLib` loads the default library of its `target`, or else each `lib`
entry: an unset, null, or empty `target` is ES2025 (`lib.es2025.full.d.ts`),
`es6` and `es2015` select `lib.es6.d.ts`, and an empty `lib` list selects no
library. `/// <reference lib>` directives in any file, libraries included, add
the libraries they name, so selection is a transitive closure; `noLib` ignores
every such reference. `lib` entries are library names in any case, not file
names; a reference also accepts the file a name selects, such as
`lib.es2015.promise.d.ts`. An unknown or empty library reference is an
`UnresolvedImport` at its value, tolerated where other missing references
are: where the reporting decision (Binding model) does not report it, as in a
library or dependency, a declaration file under `skipLibCheck`, or after a
`@ts-ignore` or `@ts-expect-error` line.
`no-default-lib="true"` only makes TypeScript ignore its whole directive; it
does not affect library selection. With `libReplacement`, each library file
except `lib.d.ts` looks up its `@typescript/lib-*` package as a `require` from
the configuration's directory (`lib.dom.iterable.d.ts` is
`@typescript/lib-dom/iterable`), and an installed package replaces the file.

The declarations are the pinned compiler's own, vendored in
`vendor/typescript-7.0.2-lib` and located through the checkout the tool was
built from: no compiler is searched for or run, nothing is downloaded, a
missing or unreadable library file is an `Io` failure, and moving the checkout
requires rebuilding. Library paths in results are real, readable files.
Libraries come first in program order, ordered as TypeScript orders them, and
are read as CommonJS scripts without a `package.json` lookup. They are never
searched and never external, even when a replacement package supplies them:
their declarations join groups, so `interface Array` merges with a project's
`declare global { interface Array<T> { ... } }`, while their own occurrences
are not reported and their bindings are checked leniently, like dependencies.
Library reads and replacement lookups are captured inputs, so a session
refreshes when an installed replacement changes.

Module resolution implements TypeScript 7's `bundler` (the default), `node16`,
and `nodenext` modes: importer format, per-request resolution mode, extension
substitution, directory and extensionless rules, `paths`, `rootDirs`,
`moduleSuffixes`, `node_modules` and `@types` lookup, package `exports`,
`imports`, `typesVersions`, `types`/`typings`/`main`, self-name imports,
`customConditions`, and `resolution-mode` on type-only imports and import
types. Relative specifiers, paths, and directives accept `\` as a separator.
A cyclic package `imports` chain is unresolved, and one longer than 1,024
redirects fails loading as `Unsupported` wherever it appears, since that is
this tool's limit rather than an input TypeScript rejects. A name with a NUL
character names no file, so it is not looked up on disk. A file's identity is its real path unless `preserveSymlinks` is set, so
a file reached through a symlinked directory is one input where TypeScript can
create two. Exact ambient modules (`declare module "x"`) win over files for
non-relative names, and pattern ambient modules (`"*.css"`) satisfy any
otherwise unresolved name: the one with the longest text before its `*`, the
first declared on a tie, in program order, as in TypeScript's
`findBestPatternMatch`. A name with more than one `*` is no pattern.
An `export *` route whose export of a name fails to resolve still exports it:
alone, its failure is the result; beside another route, the name is
ambiguous, as TypeScript reports TS2308.

A file's triple-slash `<reference ...>` directives, `@ts-check` and
`@ts-nocheck` pragmas, and JSX pragmas follow TypeScript 7: only spaces and
tabs separate their parts, a `<reference` tag needs no closing `/>`, the last
of a repeated attribute wins, and a present `types`, `lib`, or `path`
attribute, even an empty one, decides the directive in that order. A
`@ts-check` or `@ts-nocheck` pragma is a leading `//` or `///` comment whose
text after spaces or tabs is `@` and that name, matched without case and
ending before any character other than a letter or `-`; the last one decides,
and a block comment is none. A `#!` line may follow a byte order mark.

An enumeration option (`module`, `moduleResolution`, `moduleDetection`, `jsx`,
`target`, and each `lib` entry) is matched in any case, after Go's
`strings.ToLower`, and an empty string leaves it unset; an unknown value is a
`Config` error, as are `lib` with `noLib` (TS5053) and a `lib` file name in
place of a library name.

`types` defaults to `[]`; list packages, or use `"types": ["*"]` for every type
root. An explicit `typeRoots` turns off the `node_modules` fallback for type
references. `moduleDetection` defaults to `force` for Node.js `module` settings
and `auto` otherwise. Options that do not affect the program graph or binding
are ignored.

A file that resolution finds for a module request is a program input, and
what the request names, only where TypeScript admits it for the importer
(`GetResolutionDiagnostic`): a `.tsx` file needs `jsx`, a JSON file
`resolveJsonModule`, and a declaration file found for an arbitrary
extension, such as `styles.d.css.ts` for `./styles.css` or `data.d.json.ts`
for `./data.json`, `allowArbitraryExtensions` or a declaration-file importer.
Otherwise the request is unresolved (TS6142, TS7042, TS6263), even where
another route makes the file an input, and a pattern ambient module such as
`"*.css"` may still satisfy it; the only exception is a `.tsx` file that is an
input by another route, such as a root, which the request then names. The
same file named with its own extension, as `./styles.d.css` or a package's
`types`, is an ordinary declaration file. An empty module specifier is never
resolved.

JSON modules are program inputs under `resolveJsonModule`, whose default
follows TypeScript 7.0.2: on for `module` `node20` and `nodenext`, off for
`node16` and `node18`, and otherwise on with bundler resolution, the default.
Then a `.json` specifier resolves to `<name>.d.json.ts`, then `<name>.json`,
including through `paths`, package `exports`, `main`, and subpaths; an
extensionless specifier never adds `.json`. A `<name>.d.json.ts` found first
is admitted as above, so without `allowArbitraryExtensions` a source file's
request is unresolved rather than naming `<name>.json`. A `files` entry or
`/// <reference path>` naming a `.json` file must then exist; without the
option, its extension is unsupported, as for `.css`, and a `.json` specifier
is unresolved, so a pattern ambient module such as `"*.json"` may satisfy it.
An `include` spec selects JSON files only when it ends in `.json`, after the
other matches. A JSON file is always a module and is parsed as JSON by its
extension in any case; it declares no names, its own `/// <reference>`
directives are followed, and, since TypeScript checks no JSON file, a missing
target there is not an error. Its only export is `export=`, its value: a
default, namespace, `import =`, or `{ default as x }` import, even beside a
`"default"` key, names that value. Any other name selects a property of the
value's type, as a named import, a re-export, or a qualified name does
through a module whose `export =` is the value or a property of it
(`getExternalModuleMember`): a key of an object, compared exactly, including
an empty key, or, with standard libraries, a member that the global `String`,
`Number`, `Boolean`, or `Array` interface declares for a string, number,
boolean, or array, without `Object`'s members. The value, its properties, and
such members are values only, not reference targets, so queries at those
names are `UnsupportedTarget`; a name the type lacks, a name of `null` or of
an empty file, a JSON value used as a type or namespace, and an import type's
`default` (`import("./data.json").default`) are unresolved. Two `export *`
routes to one property or member are one export; to different ones, the name
is ambiguous. `export *` from a JSON module re-exports nothing, and an import
of a JSON value beside a same-named local value is a `Binding` error
(TS2440). A JSON file is searched, but its keys are string literals, so no
occurrence lies in it, and explicitly navigating one is unsupported. A JSON
file that does not parse fails loading as a `Syntax` error at the first error
this tool's parser meets, which is TypeScript's first diagnostic except where
noted below.

Rejected configurations: `baseUrl` (removed in TypeScript 7), `moduleResolution`
`node`, `node10`, or `classic`, `module` `amd`, `umd`, or `system`, `target`
`es5` (removed in TypeScript 7, TS5108), `noResolve`, `noDtsResolution`,
a non-empty `references`, and JavaScript roots or reference targets,
whether or not the file exists. As in TypeScript, whether an input has a
supported extension follows the file system's case policy, so `A.JS` is
JavaScript on a case-insensitive file system; other name rules use the exact
spelling, so `G.D.TS` is an ordinary module rather than a declaration file, and
JSX follows a `.tsx` extension in any case.

## Compatibility limits

- One configuration. Project references are not followed; a configuration with
  non-empty `references` is rejected.
- Inputs are `.ts`, `.tsx`, `.mts`, `.cts`, their declaration files, and JSON
  modules. JavaScript, JSDoc binding, and asset modules are unsupported
  when resolution needs them. In a dependency file, an import or
  `/// <reference path>` that reaches such an input is skipped instead.
- Standard library declarations come from the checkout the tool was built
  from; there is no installation override or relocation.
- `with` statements and other syntax without a binding rule are rejected with
  an `Unsupported` error naming the construct.
- Modules and exports are keyed by their exact names, which must be valid
  Unicode. A module specifier, ambient module name, or string-literal import or
  export name with an unpaired UTF-16 surrogate, such as `"\uD800"`, is
  rejected with an `Unsupported` error at its string literal rather than merged
  with the other names that a replacement character would make equal.
  TypeScript keeps such names distinct.
- File and directory names must be Unicode. On a file system that allows
  other names, such a name that file selection or `"types": ["*"]` would use
  is `Unsupported` rather than converted to a different name.
- Lexical searches do not infer receiver/member identity. Semantic member
  searches are a separate coverage baseline; their failures and deliberate
  differences are described below and in [types.md](types.md).

## Loading and lexical differences from TypeScript 7.0.2

The semantic oracle is TypeScript 7.0.2's language service (see
[validation](validation.md#reference-semantics)). Its results are normalized
to this model before comparison. Normalization keeps every program file's
locations, so it cannot hide a missing or extra occurrence; it names files by
canonical path, as this tool does, unless `preserveSymlinks` is set:

- TypeScript gives every same-named declaration in one table a single symbol.
  This tool keeps separate symbols per meaning unless the declarations merge,
  so `interface X` and `declare var X` are separate groups.
- TypeScript reports several groups for aliases of one symbol; this tool
  reports one group for the underlying symbol. TypeScript also creates a
  temporary symbol for each `import * as ns`; groups are identified by their
  declarations instead.
- A value use of a type-only alias is unresolved here; TypeScript reports an
  error but still resolves it. When star re-exports reach one declaration by
  both a type-only and an ordinary route, the ordinary route makes it a value
  here; TypeScript keeps the first route and may report that error.
- A name that two `export *` sources supply with different targets is
  ambiguous and unresolved here; TypeScript reports TS2308 and resolves it to
  the first source. An augmentation that declares such a name becomes the
  module's own export here; TypeScript merges it into the first source's
  symbol.
- Every module and global augmentation of one declaration merges into one
  identity here, and every alias resolves against the merged declarations.
  TypeScript 7.0.2 keeps the alias targets it resolved while merging: an
  `import`, `export { X } from` specifier, or UMD global (`export as
  namespace`) resolved during a merge can keep the unmerged declaration, an
  import through a pattern ambient module keeps the pattern's own unmerged
  export, and augmentations that reach one declaration through different
  modules' exports can each clone it, so its declarations split into several
  groups depending on program order. Likewise, when the export an
  augmentation names is an alias in a cycle (TS2303), the augmentation's
  declaration replaces it, and the cycle's aliases and their importers then
  resolve to that declaration here; TypeScript leaves them unresolved.
  Projects in `fixtures/reference-exceptions` record the compiler's answers
  beside this tool's for each of these and for the ambiguous star export
  above; `tests/reference_exceptions.rs` checks this tool's, and a real-oracle
  test in `tools/semantic-oracle.test.mjs` the compiler's.
- When a later augmentation of a pattern-matched specifier declares a name
  that only the pattern declared, TypeScript merges it into the pattern's own
  symbol, so the pattern's declaration joins that augmentation's group; here
  every augmentation of the specifier extends that specifier only, and the
  pattern's declaration stays with the other specifiers' references
  (`pattern-later-augmentation` in `fixtures/reference-exceptions`).
- In a dependency file that declares a name both exported and local (TS2395
  or TS2383, tolerated there), the declarations merge into one symbol, so its
  group also lists the local declarations; TypeScript's export symbol has only
  the exported ones. The same holds for an exported enum, class, or function
  merged with a local namespace in spaces that do not conflict, such as an
  exported enum and a local namespace of types: TypeScript gives the export its
  own symbol, here they form one group.
- A JSON module's value and properties are not reference targets here;
  TypeScript gives an import of them a group of the import's own
  occurrences, and a property access such as `data.a` a group that includes
  the key in the JSON file. The oracle classifies both as members. An
  interface that an augmentation merges beside such a value is a separate
  symbol of the type meaning here, as for other declarations of several
  meanings, so a type use of the name denotes the interface; TypeScript's one
  symbol is a property, which the oracle classifies as a member
  (`fixtures/json-probes/json-augmentations.json`).
- The members a JSON string, number, boolean, or array has are those that the
  declarations of its global library interface declare. Where one of those
  declarations extends other types, a name none declares is unsupported here;
  TypeScript also finds the inherited members and leaves other names
  unresolved. A member is one target per interface and name, so two arrays'
  `length` through two `export *` routes is one export here; TypeScript
  instantiates an array's members per element type, so arrays of different
  element types make the name ambiguous there (TS2308). A library member's
  own type is not inferred, so a name selected from it, as through
  `export = length`, is unsupported; TypeScript resolves it from the member's
  type (`fixtures/json-probes/linking.json`).
- Checker diagnostics that do not change what a name denotes are not reported,
  as for other checker rules: the import attribute and named-import rules for
  JSON under `node18` and later (TS1543, TS1544), `export *` from a JSON module
  (TS2498), an `import =` under ECMAScript module output (TS1202), and TS6142
  for an import of a `.tsx` file without `jsx` that another route makes a
  program input, which the import still names.
- A JSON file with a syntax error fails loading, as other inputs do;
  TypeScript reports the diagnostic and still binds what it parsed.
- A syntax error inside a token, such as an invalid escape, digit, or numeric
  separator, or an unterminated string or comment, is reported at the
  token's start; TypeScript reports it at the offending character, as
  `"\00"` at its `\`. Where TypeScript's error recovery parses on and its
  JSON validation then reports an earlier node, such as a JSON member that is
  not a property assignment (`{get "a": 1}`, `{default "a": 1}`) or a value
  that is not JSON (`{"a": 1 ? }`), this tool reports where its parser stops,
  later in the file (`fixtures/json-probes/parsing.json`).
- Standard library files are never searched. TypeScript's own
  find-all-references also reports occurrences inside them; this tool, like the
  semantic oracle's model, reports their declarations in groups but no
  occurrences from them.
- Libraries of equal program-order priority, which only replacement packages
  and a `.full` default can be, keep TypeScript's depth-first discovery order
  here; TypeScript sorts more than twelve libraries with Go's unstable sort,
  which may reorder them. Only file order differs.
- A file reached both as a library and through another route, such as a
  project file naming a vendored or replacement library by
  `/// <reference path>` or import, takes the provenance of the route that
  reaches it first, as TypeScript's loader takes it from the task it starts
  first: a root file, then a root library, then breadth first, each file's
  routes in its subtask order. Where two routes are equally deep, TypeScript's
  parallel loader starts either first, so its choice varies between runs; this
  tool takes the earlier one in program order
  (`tests/reference_libraries.rs`).
- Replacement packages are resolved from the configuration's real directory;
  TypeScript uses the directory as the configuration path spells it, so
  through a symbolic link it can find a package beside the link that this
  tool does not (`tests/reference_libraries.rs` and, for the compiler,
  `tools/semantic-oracle.test.mjs`).
- An unknown library reference is reported without TypeScript's spelling
  suggestion (TS2727).
- `undefined`, `arguments`, and `globalThis` are built-in non-reference
  targets. A missing export of a namespace, module, or `globalThis` qualifier is
  unresolved; any other unresolved `a.b` name is a property. Receiver types are
  not inferred, so `value.member` is a property even where TypeScript resolves
  it through a variable's `typeof namespace` type.
- A named import from a shorthand ambient module (`declare module "x";`)
  denotes the module, as in TypeScript; a member reached through one is
  unresolved. Such an import resolves in a type position too, where
  TypeScript reports TS2709. A default import of a declaration file or ambient module with
  neither a default export nor an `__esModule` export denotes the module
  object, as TypeScript's synthetic defaults do; an ordinary module without
  one has no default.
- TypeScript stores an import and a same-named local declaration of another
  meaning in one table symbol; uses select the meaning they need, and the
  import clause name denotes only the imported declaration.
- Non-reference syntax is unsupported even where TypeScript resolves it: a
  binding-pattern key such as `default` in `const { default: x } = await
  import("m")`, `const` in `as const`, `global` in `declare global`, a `this`
  parameter, `this` in a type query such as `typeof this.x` (TypeScript
  resolves it to the enclosing class), and a tuple member label.
- A constructor parameter property's name belongs to the parameter's group;
  TypeScript resolves the name itself to the property.
- Invalid declaration arrangements that TypeScript binds but its checker
  rejects are `Binding` errors here where reported (Binding model): duplicate
  implementations (TS2393), a function merged with a non-ambient class
  (TS2813/TS2814), a repeated type parameter (TS2300), a hoisted `var` past a
  block's `let` (TS2481), a name declared both exported and local in one
  container in overlapping declaration spaces (TS2395, or TS2383 at the
  overloads whose export differs from the implementation's or the first's,
  so an enum and a namespace of types may differ),
  an instantiated namespace before or in another file from the class or
  function it merges with (TS2434/TS2433; a namespace of const enums only
  counts under `preserveConstEnums`, `isolatedModules`, or
  `verbatimModuleSyntax`), and merged classes or interfaces whose type parameter
  names disagree by position or omit a parameter that no declaration gives a
  default (TS2428), and an import whose target shares a meaning with a
  same-named local declaration, even when it resolves back to that local
  through its own module (TS2440). Constraints and default types are not
  compared, including between a declaration and its module augmentations.
- An unresolved side-effect import fails the project, as TypeScript reports
  TS2882, unless `noUncheckedSideEffectImports` is false.
- Binding errors and missing inputs in dependency and standard library files
  are not reported (Binding model), so a name that depends on such a missing
  input is unresolved, and conflicting declarations there merge; TypeScript
  reports them unless `skipLibCheck` or `skipDefaultLibCheck` covers the
  file (`fixtures/json-probes/reporting.json`).
- An unresolved module or reference directive leaves the names that depend
  on it unresolved instead of failing the project where TypeScript reports no
  error: in a file it skips, such as a declaration file under `skipLibCheck`
  or a file under `// @ts-nocheck`, or after a `@ts-expect-error` or
  `@ts-ignore` comment directive (as for binding errors). The directive
  rules follow TypeScript 7.0.2 literally (`crate::pragmas`, from
  `processCommentDirective` and `isCommentOrBlankLine`). A directive is a
  `//` comment, after any further slashes and then only spaces and tabs, or a
  block comment's last line, after spaces and tabs and then any run of `/`
  and `*` and again spaces and tabs, that continues with `@ts-ignore` or
  `@ts-expect-error`, case-sensitively and as a prefix (`@ts-ignored`
  counts); any other whitespace, such as a no-break space, vertical tab, or
  form feed, ends it. It covers the next line after the line it starts on,
  and the lines after that while every line between is blank or a comment
  line in TypeScript's narrow sense: after spaces and tabs only, the line is
  empty up to CR, LF, or the end of the file, or begins with `//`. This is
  not the ECMAScript notion of a blank line: a line holding a no-break space,
  vertical tab, form feed, or byte order mark, or an empty line that U+2028
  or U+2029 ends, stops the directive, and the error fails the load as
  `tsc` reports it (`fixtures/json-probes/suppression-lines.json` and
  `suppression-directives.json`). Such a comment counts anywhere between
  tokens, including inside an import's braces or after code on its line, but
  not inside a string, template, regular expression, or JSX text. A missing automatic JSX runtime is reported once per
  file, at its first JSX tag in source order; TypeScript uses the first tag it
  checks, which can be later in source when a function expression's body is
  checked late.

TypeScript 7.0.2 defaults `types` to `[]`: `@types` packages are included only
when listed in `types`, or all of them with `"types": ["*"]`. The design assumed
automatic discovery when `types` is absent; the implementation follows the
compiler.

## Semantic type and member-query differences

The semantic owners implement the contract in [types-design.md](types-design.md),
including its deliberate differences from TypeScript. This supersedes lexical-only
member exclusions for semantic queries. The [completion checkpoint](types-completion-correctness-2026-10-13.md)
records the current validation status; this section is not a delivery-complete claim.

- Coverage names its operation: existing lexical search versus semantic member
  search. A complete lexical result does not promise all receiver/property
  occurrences in the compiler's broader reference operation. Bare-name results
  disclose matching catalog member selectors that were not searched.
- Member candidates use exact decoded keys instead of 7.0.2's raw-text bounded
  substring optimization, covering escaped and equivalent numeric spellings.
  Compiler syntax/meaning eligibility still applies; arbitrary computed-key
  variable tokens are not wildcard property candidates.
- Member identity uses the resolved combined early/late member, independent of
  cold language-server query order. This also permits established literal-name
  uses of computed-only members that the cold server omits. Declaration origins
  and eligible occurrence tokens remain different questions.
- A parameter property's name preserves lexical parameter selection; its first
  property-producing modifier is a member anchor. Dotted namespace candidates
  anchor the actual selected segment. Ambiguity anchors must round-trip through
  that operation's selection owner.
- Q4 normalizes declaration/alias meanings for useful inspection. Both type-only
  import/export spellings follow one table; type-reference name tokens select
  their type syntax. Explicit closing delimiters can select call/access/tagged
  results. These product anchors use node-level type operations rather than
  claiming equality to every raw compiler position query. Unsupported catalog
  forms and positions inside JSON remain explicit target failures.
- Checking starts fresh per semantic request, resolves containing calls
  outermost-first, and uses a defined candidate/expansion schedule. It does not
  inherit earlier queries' caches or promise the same limit/context history as
  a complete `tsc` diagnostics pass. Oracle fixtures must record raw observations
  and the exact normalized sequence; a diagnostics request alone does not settle
  the API checker. Compiler constants, reset points and operation outcomes remain
  unchanged inside that schedule.
- Compiler-process numeric IDs do not affect semantic identity. Cache keys
  distinguish opaque symbol/type/node identities from exact user-name bytes;
  adjacent name bytes still concatenate as in the native operation. Generated
  private/unique keys retain identity and exact textual components with the
  request-local ordering defined in the
  [evaluation baseline](types-design.md#evaluation-order-baseline). This
  deliberately removes native collisions between encoded IDs and property
  names, and decimal-ID ordering in generated names. The
  [identity review](types-native-identity-review.md) preserves compiler evidence
  that unrelated process allocations can otherwise change a final type. The
  implementation and final-query proof remain separate completion obligations.

The design also defines product call/test filters, bounded presentation,
unsupported-flow/JSX gaps and typed failures. These limitations are not a claim
that TypeScript itself cannot analyze those sites.

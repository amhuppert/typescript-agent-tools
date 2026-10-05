# Find references design

Add project-wide reference lookup to `tsparse` through an immutable
`ReferenceProject`. Its caller supplies a `tsconfig.json` and selects a symbol by
source position or exact name. The implementation owns project loading, binding,
alias resolution, and result grouping. Callers never coordinate compiler passes
or inspect scope tables.

This is a proposed design for implementation. The interview established the
feature scope; the API, result format, compatibility limits, and failure policy
below are recommended design decisions, not implemented behavior.

## Scope and operating envelope

The confirmed scope is one configured TypeScript project, inherited settings,
file selection, local import aliases, and both query entry points. Resolve
variables, parameters, functions, named types, class names, and import/export
aliases with TypeScript scope and shadowing rules. Defer ordinary property and
method lookup and traversal through linked TypeScript projects.

Assume a local, synchronous CLI invocation over files on disk, with no server,
watcher, concurrent updates to an index, or persistent index. Keep execution in
Rust and preserve the existing parser API and AST JSON contract. No latency or
project-size target was specified; measure full-project indexing before adding
parallelism or caching.

The result is source navigation information within the selected project. It is
not a rename edit set or a promise about runtime uses through reflection, strings,
or dynamic code. A missed or incorrectly grouped source occurrence matters, so
resolution failures must remain distinguishable from an empty successful search.

## Constraints established by the code

| Current contract | Design consequence |
| --- | --- |
| [`Ast` borrows its source](../src/ast.rs), and `NodeId` is arena-local. | Own source text and semantic records in the project; do not build a self-referential collection of sources and borrowed ASTs. |
| `Ast::nodes()` includes discarded parse nodes. | Visit only nodes reachable from `root_id()`, using an explicit traversal stack. |
| Syntax fields can share children, such as `attributes` and `assertClause`. | Enumerate semantic children once. A fallback walker deduplicates arena IDs and does not count compatibility aliases twice. |
| Identifier spans include leading trivia; names use compiler `escapedText`. | Extract exact token spans and decoded identifier names in one place. Do not compare raw source spelling or emit trivia as a reference. |
| Rust spans are UTF-8 bytes; JSON offsets and diagnostic columns use UTF-16. | Keep byte ranges internally and centralize validated conversion at the query/result boundary. |
| [`src/main.rs`](../src/main.rs) currently reads one file or stdin. | Add an explicit reference mode; put project and semantic work below argument parsing. |
| [`tools/reference-oracle.mjs`](../tools/reference-oracle.mjs) is a TypeScript 6.0.3 syntax oracle. | Add a separate semantic oracle. Existing AST conformance does not establish binding correctness. |

## Public operation and ownership

The interface comment should state the complete operation before implementation:

```rust
/// An immutable reference index for one configured project.
///
/// Loading reads each participating file once and resolves the supported
/// declaration and module relationships. Queries perform no filesystem I/O.
/// Results describe those captured contents, not later edits on disk.
/// Scope tables, aliases, and internal IDs never escape this object.
pub struct ReferenceProject { /* private */ }

impl ReferenceProject {
    pub fn load(config: &Path) -> Result<Self, ProjectError>;
    pub fn find_references(&self, query: ReferenceQuery)
        -> Result<ReferenceResult, QueryError>;
}

pub enum ReferenceQuery {
    At { file: PathBuf, position: SourcePosition },
    Name { name: String, file: Option<PathBuf> },
}

/// One-based line and one-based UTF-16 column.
pub struct SourcePosition { pub line: usize, pub column: usize }
```

These are signature sketches, not a request to expose the private index layout.
`load` returns a usable object or a diagnostic; there is no partially initialized
state or public `parse`, `bind`, and `link` sequence. Results own their small
location/name records, so they can outlive the project. Multiple library queries
reuse the same immutable index, which is already useful for the two entry points.

```mermaid
flowchart TD
    CLI[CLI arguments and output] --> API[ReferenceProject]
    API --> Project[Project configuration and module files]
    API --> Binding[Bindings and symbol relationships]
    Binding --> Syntax[Existing syntax parser]
    API --> Query[Symbol selection and occurrence index]
    Project --> Source[Source text and locations]
    Binding --> Source
    Query --> Source
```

Use a public `references` module with private implementation modules. Start with
the following ownership boundaries; file splitting is secondary to the contracts.

| Owner | Knowledge it hides | What consumers receive |
| --- | --- | --- |
| `references::project` | JSONC configuration, inheritance, path provenance, file selection, resolution modes, package metadata, and file identity | Captured files, effective binding settings, resolved module edges, and which files belong in results |
| `references::binding` | AST field roles, scope creation, declaration compatibility, meaning-aware lookup, namespace exports, and alias relationships | A completed semantic index with symbol identity for supported occurrences |
| `references::query` | Position selection, exact-name candidate selection, reverse occurrence lookup, deduplication, and stable ordering | Owned reference groups |
| `references::source` | Source ownership, exact token locations, line breaks, and byte/UTF-16 conversion | Validated locations and source access by private `FileId` |
| `references::mod` | Construction order and consistency between these parts | The public operation above |
| Existing CLI boundary | Flags, printing, output files, and exit codes | The existing command behavior plus reference mode |

Do not create a public scope-graph framework, resolver plugin system, injectable
filesystem hierarchy, or generic compiler pipeline. There is one implementation
and one filesystem. Unit tests can bind source strings; loader tests can use
temporary project directories. Keep the binder's passes and shared tables in one
owner even if they later occupy several private files.

## Command and result contract

Use an additive flag rather than reserving a subcommand that could collide with
an existing positional filename:

```sh
tsparse --references --project ./tsconfig.json --at 12:8 src/app.ts
tsparse --references --project ./tsconfig.json --name run
tsparse --references --project ./tsconfig.json --name run src/app.ts --pretty
```

`--project` is required in reference mode. It names a configuration file, without
ancestor discovery. `--at LINE:COLUMN` and `--name NAME` are mutually exclusive.
Position queries require a file. For name queries, an optional file narrows the
candidate bindings, while returned references still span the project. Paths in
arguments are relative to the working directory. `--pretty` and `-o` retain their
existing meanings. Reject stdin, `--check`, and `--tsx` in reference mode; project
configuration and filename determine the input set and syntax mode.

Name lookup is exact and case-sensitive on decoded identifier text. It is not
substring search. Two shadowed declarations with the same name yield separate
groups. Searching an alias name selects the underlying symbol through that alias;
two matching aliases of the same symbol yield one group. Position and name queries
share the same grouping operation rather than implementing separate resolvers.
Candidates include declaration names, local import names, and explicit exported
alias names in the search files. An exported alias is discoverable even when that
spelling introduces no local lexical binding.

A position must fall within an identifier token, start inclusive and end
exclusive. Whitespace and punctuation return an empty result; an out-of-range
line/column or a column inside a surrogate pair is an invalid query. Selecting a
known property/method name returns `UnsupportedTarget`, rather than a misleading
empty reference list. An unresolved identifier returns `UnresolvedTarget`.
An explicit query file must belong to the search files; reject a file outside
that boundary rather than adding it to the already constructed project.

Use one result shape for either entry point:

```text
ReferenceResult {
  project: absolute path to the selected tsconfig,
  groups: [
    {
      declarations: [ NamedLocation ],
      occurrences: [ { name, location, roles } ]
    }
  ]
}
```

`NamedLocation` contains `name` and `location`. A location contains `file`, `start`,
and `end`; positions are one-based lines and UTF-16 columns with an exclusive end.
File paths use `/` separators and are relative to the selected configuration's
directory, including `../` when necessary. Internal file identities do not leak.

`declarations` identifies the underlying symbol's declarations, including valid
merged declarations. `occurrences` includes declarations in the search boundary,
uses, and import/export name tokens. Roles are a small set drawn from
`declaration`, `use`, `import`, and `export`; one token may have several roles.
Do not add read/write or rename-safety classifications to this version.

Each file/range appears once per group, combining roles. Sort locations by file
and start position, and groups by their earliest declaration location. This keeps
results reproducible without persistent symbol IDs. An ordinary absence is
`groups: []`, with exit code 0. The CLI serializes the domain result directly; it
does not reproduce filtering or alias policy.

## Binding model

Keep private, project-local `FileId`, `ScopeId`, `SymbolId`, and `OccurrenceId`
newtypes. During parsing, `NodeId` remains paired with its originating AST. Copy
only semantic facts needed after that AST is dropped: declarations, scope edges,
occurrences, qualified-name paths, and module directives. Retain source text and
its position index, not every project's full syntax arena.

Use a scope's name entry to represent its value, type, and namespace meanings.
Lookup asks for the meaning required by the occurrence and searches the enclosing
scopes accordingly. A same-named declaration in the wrong meaning must not
automatically shadow the required meaning. A class or enum can introduce more
than one meaning; an unrelated type alias and variable with the same spelling
must not become one reference group merely because they share a name.

Separate three concepts:

1. A **binding** is a name visible in a particular scope and meaning.
2. A **symbol** is the declaration identity to which an occurrence resolves.
3. An **alias edge** relates an import/export binding to a target symbol, with
   the allowed meanings and the alias's own declaration location.

Declaration merging is an explicit compatibility operation owned by the binder.
It handles valid overloads, interface merges, and supported namespace/class/
function combinations, preserving every declaration. Invalid duplicate bindings
are a binding error, not an arbitrary choice of the first declaration. Namespace
merges share exported declarations, while non-exported declarations remain local
to their declaration block.

Keep alias edges directed. Do not use a global union of same-spelled identifiers,
and do not erase type-only restrictions while following an alias. Cache resolved
alias targets within the immutable project once the graph is complete.

### Building the index

The construction sequence is private and has these invariants:

1. Load effective configuration and discover root files. Discover additional
   module/directive dependencies as files are parsed; canonical file identities
   prevent repeated loading.
2. For each file, parse once, visit the final tree, and record all declarations,
   scopes, pending uses, export entries, and exact token locations. Resolve no
   forward use by scanning only declarations seen so far. Drop the AST after its
   owned facts have been captured.
3. Complete project/global declaration tables and export relationships. Resolve
   re-export cycles using an explicit worklist or strongly connected components.
   An edge cycle is not itself an error; a required name with no declaration
   target, or conflicting star-export targets, remains unresolved or ambiguous.
4. Resolve pending occurrences by lexical meaning or a proven namespace export
   path. Record either a symbol or an explicit non-reference/unresolved/
   unsupported classification. Build the exact-name and reverse occurrence
   indexes from the same records.
5. Freeze the complete project before serving a query.

Scope discovery and lookup are separate passes within the binder, not separate
public modules. This handles forward declarations and shadowing without making
callers understand hoisting. A use in the temporal dead zone still resolves to
the inner binding for navigation; runtime initialization checks are outside scope.

### Syntax rules that require deliberate treatment

| Syntax | Binding obligation |
| --- | --- |
| `var`, `let`, `const`, `using`, and `await using` | Distinguish function/global and block bindings, including loops, switch clauses, catch bindings, and class static blocks. |
| Functions, arrows, and named expressions | Model parameters, generic parameters, the inner expression name, and body declarations. Parameter initializers cannot simply see all body-local declarations. |
| Destructuring | Introduce bindings from patterns; visit default/computed expressions as uses. Property keys are not automatically identifier references. |
| Object shorthand and assignment patterns | `{value}` uses the lexical value; `{value: other}` uses `other`. Assignment targets and declaration patterns have different roles. |
| Types | Type references use type/namespace lookup; `typeof value` in a type uses value lookup. Handle generic, mapped, and conditional `infer` scopes, including the true-branch boundary. |
| Classes and interfaces | Bind their names and generic parameters. Traverse method bodies, annotations, decorators, and computed names in their correct scopes even though member searches are deferred. |
| TSX | Component identifiers and expression containers can be lexical uses. Intrinsic tag names and attribute names are not local variables. |
| Global scripts and namespaces | Distinguish module-local declarations from project globals. Do not assume every file is a module, or every namespace block shares all locals. |
| Non-reference identifiers | Labels, noncomputed property names, import attribute keys, and similar syntax must not enter lexical lookup. |

Own these rules alongside scope construction. A generic walk that treats every
`Identifier` as a use cannot meet the contract. Conversely, excluding a method
name must not exclude lexical uses inside that method.

Known transparent syntax may recurse without special treatment. Unknown
scope-bearing or declaration-bearing syntax must produce an unsupported-feature
diagnostic until a behavior test and binding rule exist. This prevents parser
coverage from being mistaken for reference coverage.

### Aliases and qualified names

Given:

```ts
// api.ts
export function run() {}

// barrel.ts
export { run as execute } from "./api";

// app.ts
import { execute as start } from "./barrel";
start();
function local(start: () => void) { start(); }
```

Selecting the top-level `start` or searching for `execute` returns the `run`
symbol, the import/export occurrences, and the top-level call. The parameter and
its call form a different group. Searching `start` without a file restriction
returns both groups. Ordinary value flow, such as `const copy = run`, does not
make `copy` an alias of `run`.

Support named/default imports, named re-exports, star re-exports, namespace
imports/exports, type-only aliases, and TypeScript import/export assignments where
the syntax establishes an alias. Do not equate an arbitrary default-export
expression with all identifiers it contains. Anonymous default declarations need
a synthetic default-export symbol anchored at their declaration, not a fabricated
identifier occurrence.

`ns.run` may resolve `run` when `ns` is proven to be an imported or declared
namespace. This is export-table lookup, requiring no receiver-type inference.
The lexical receiver in `object.run` is still a reference to `object`, but ordinary
object properties, class members, enum members, and inferred receiver types remain
outside target selection. The same distinction applies to qualified type names.

## Project configuration and resolution

The project owner is a substantive part of this feature. It cannot be replaced
with a directory walk and a string comparison on module specifiers.

Keep path-valued configuration entries associated with the configuration file
that declared them. Inherited `include`, `exclude`, `baseUrl`, and `paths` must not
all be reinterpreted relative to the leaf configuration. Handle JSONC comments,
trailing commas, ordered `extends` inheritance, and inheritance-cycle errors.
Keep JSONC parsing behind this owner; `serde_json` alone does not accept JSONC.

Define these sets explicitly:

- **Roots:** TypeScript files selected by the effective `files` and
  `include`/`exclude` rules, including declaration files and TSX.
- **Program inputs:** roots plus transitive static imports/re-exports, supported
  reference directives, and ambient type dependencies selected by `types` and
  `typeRoots`, including automatic package discovery when `types` is absent.
  An excluded file can still enter through an import, as in TypeScript.
- **Search files:** roots and their locally resolved source/declaration
  dependencies. Installed package declarations can establish identity without
  recursively reporting every reference inside dependency packages.

Installed package declarations participate in resolving imported names; a group's
underlying declaration location may consequently be outside the search files.
Package declaration files reached only as resolution dependencies are not
themselves name-discovery candidates. Local import bindings for those symbols
remain candidates, and project-local occurrences are returned.

The recommended initial compatibility profile covers `.ts`, `.tsx`, `.mts`,
`.cts`, and their declaration counterparts; relative imports, configured local
aliases, and installed packages with TypeScript declarations. Implement the
effective TypeScript 7 `node16`, `nodenext`, and `bundler` resolution behavior,
including importer format, extension substitution, package `exports`/`imports`,
type conditions, and namespace/default export distinctions. Package names are
not interchangeable with filesystem paths. Do not fall back from an explicitly
unsupported resolution mode to an approximate algorithm.

Honor settings that affect this graph or binding, including module detection,
package format, `types`/`typeRoots`, and symlink identity. Emit-only settings can be
ignored. Record the supported configuration surface next to resolver tests; any
new option gets its interpretation in this owner, not a CLI branch. A config's
mere presence of an irrelevant option should not reject an otherwise supported
project.

There are explicit first-version limits:

- Do not follow project references. A project requiring that traversal receives
  an unsupported-project diagnostic rather than an apparently complete workspace
  result.
- JavaScript/JSDoc binding, JSON and asset module declarations synthesized by
  tooling, and arbitrary `require`/dynamic-import value flow need later support.
  Report an unsupported input when such a dependency is necessary to resolution.
- Do not invent built-in standard-library symbols. References to ambient names
  without a declaration among the supplied program inputs are unresolved targets.
  Automatic TypeScript standard-library provisioning is deferred; this makes the
  supported universe narrower than the editor's complete default program. An
  explicit `lib` setting or reference directive requiring an unavailable library
  must receive an unsupported-input diagnostic, rather than being ignored.
- Resolve ambient module declarations and global augmentations supplied by the
  program before declaring an import missing. A literal import can be satisfied
  by an ambient declaration without an on-disk module.

These limits belong in command help and acceptance fixtures. They are compatibility
boundaries of the proposed implementation, not claims of full compiler parity.
Changing automatic ambient-library loading later belongs in the project owner and
does not change the query API.

## Errors and resource behavior

| Condition | Proposed behavior |
| --- | --- |
| No matching name or no token under a valid position | Successful empty result. |
| Several distinct matching declarations | Successful separate groups. |
| Valid overloads or declaration merging | One symbol group with all underlying declarations. |
| Unsupported target or unresolved selected identifier | Query diagnostic; no successful empty substitute. |
| Invalid flags/position, missing file, unreadable config | Usage/I/O diagnostic, exit 2. |
| Syntax error, unsupported required semantic input, invalid binding, unresolved required import, or ambiguous required export | Project/query analysis diagnostic, exit 1 in reference mode. |

Preserve the existing parse-mode exit contract. Reference help explains its wider
meaning of exit 1. The library returns typed errors carrying path and source/config
location where available; only the CLI chooses an exit code or prints diagnostics.
Stop before emitting reference JSON if project construction fails. Do not add a
partial-results switch to the first version.

An ordinary unresolved name elsewhere in otherwise bindable code need not abort
every query: store it as unresolved and report it if selected. Missing module or
ambient-dependency inputs are different because they can change the project graph
or global binding environment. Unrelated type errors are not checked; binding
success does not claim that the whole project type-checks.

The captured file contents are authoritative for the lifetime of the index.
Reading different files is not an atomic filesystem snapshot. File disappearance
or read failure is an I/O error; the tool takes no locks and promises no protection
against concurrent edits during loading. The index is disposable navigation data,
so retries, journaling, durable IDs, and cross-process coordination add no value.

Parse files sequentially and release each AST after extraction. Retained memory
is source text plus semantic records and indexes, with only one file's parse
working set added at a time. Use worklists for graph cycles and iterative syntax
walks so the existing parser's depth safeguards are not undermined by a later
recursive walk. Cache module-resolution and alias results only inside this load.
Measure construction time, peak memory, and query time separately.

## Alternatives and reasons for the choice

| Alternative | Common caller and change cost | Decision |
| --- | --- | --- |
| Bind declarations while parsing and store semantic state on syntax nodes | Saving a traversal also couples module loading and scope rules to speculative parsing, reparsing, and a borrowed arena. Parser changes can invalidate symbol state; parse-only callers pay or gain mode switches. | Use a separate semantic pass over the final tree. The parser keeps its syntax contract. |
| Search independently for each query by scanning names and walking imports | Avoids an initial full index, but position and name searches repeatedly reconstruct identity, cycle handling, and shadowing. Negative matches are especially hard to establish. | Build one complete immutable index and reuse its selection/grouping logic. Reconsider lazy indexing only with measured need. |
| Expose loaders, scope builders, and linkers as public library operations | Flexible sequencing moves configuration and initialization invariants to every caller. Changes to one pass propagate to clients. | Expose the complete project operation; keep construction stages private. |
| Invoke the TypeScript language service at runtime | Offers established semantic behavior and lowers implementation cost, but changes distribution/runtime dependencies and the native-Rust direction assumed in the brief. | Use TypeScript for test evidence, not as the production backend. Revisit this explicitly if native compatibility cost exceeds the desired investment. |

The chosen design applies deep modules and information hiding to two difficult
decisions: what constitutes a project, and what declaration an occurrence denotes.
The CLI does not know either algorithm. A module-resolution fix changes the
project owner and its fixtures; an `infer`-scope fix changes the binder and its
fixtures; a JSON-format change stays at the result boundary. There is no reason
for those changes to require edits in all three places.

## Implementation and verification sequence

This document changes no executable behavior, so test-first implementation is not
applicable to this design change. Implementation should use behavior-level
red-green-refactor steps, with these milestones:

1. **Make semantic evidence executable.** Establish a pinned semantic oracle and
   marker-based source fixtures that compare declaration identity and occurrence
   locations. First cover shadowing, type/value meanings, and alias selection;
   these settle the representation before broad syntax coverage. An initial
   end-to-end test should fail on the missing lookup behavior after a minimal
   callable skeleton exists.
2. **Complete the local binder.** Add the public project/query shape with private
   owned records, then test the scope families above using source strings.
   Include names with escapes/leading underscores, astral characters, all supported
   line breaks, trivia, shared children, and reparsed unreachable nodes.
3. **Complete project identity.** Use temporary multi-file projects for inherited
   configuration, aliases, namespace imports, barrels, cycles, ambiguous stars,
   excluded-but-imported files, package declarations, globals, and augmentations.
   Configuration and module-resolution fixtures also compare against the pinned
   compiler's file selection and resolution behavior.
4. **Wire the CLI.** Add both entry points, consistent sorting, file narrowing,
   empty results, failure exits, and JSON output. Retain all existing parse CLI
   behavior tests. Add one full CLI fixture combining cross-file aliases and an
   identically named shadowing parameter.
5. **Broaden evidence before release.** Run generated combinations and sampled
   real projects through the semantic oracle; record unsupported cases rather
   than filtering mismatches away. Measure whole-project costs separately from
   the parser-only benchmarks, then run the registered pre-merge checks.

Assertions should name expected declaration/occurrence relationships, not private
scope counts or allocation layouts. Querying any supported occurrence of the
same symbol should give the same group. Compare aliases after normalizing the
tool's deliberate whole-symbol grouping: an editor may return several alias
groups. Do not call those output-format differences binding failures, and do not
discard missing or extra occurrence locations during normalization.

The existing API oracle is pinned to TypeScript 6.0.3, while the parser targets
TypeScript 7. A 6.0.3 language service can provide interim evidence for explicitly
compatible cases, but it cannot establish general TypeScript 7 reference parity.
Before claiming that parity, obtain a pinned TypeScript 7 semantic oracle through
its available language-service/test interface and record the version and supported
comparison surface. That interface has not been verified in this design task;
the configured 6.0.3 API was also unavailable in the current worktree.

The Rust `test` command retains its full wrapper for checkpoints and adds
[`scripts/validate/test-changed.sh`](../scripts/validate/test-changed.sh) as
`validation.commands.test.command.changed`, with `pathArgs: "paths"`. Explicit
paths must name existing top-level integration-test files, `tests/<target>.rs`,
which map to Cargo's automatically discovered test targets. Directory paths,
source files, and unsupported test layouts are rejected rather than widening the
run. Without explicit paths, the changed wrapper safely runs the full suite.

Command Center reads the registration and resolves wrapper executables from the
canonical project root (the main worktree), then runs them with the session
worktree as the working directory. Changes made only in this session therefore
do not take effect through `cctl validate`. Integrate both the `CommandCenter.json`
registration and the executable wrapper into the canonical project root through
the normal Command Center integration process before relying on scoped validation.

While developing the wrapper, a direct invocation is permitted only as the narrow
wrapper-development diagnostic described in the project-setup skill; it is local
evidence, not a registered validation verdict. After canonical integration, check
`cctl validate list` for path support, then use
`cctl validate run test --queue-if-busy --require-match -- tests/references.rs`
for the TDD loop. Verify that the run selects the intended test target before
claiming a file-scoped result.

## Main implementation risks

Module resolution and TypeScript's less common binding scopes are substantial
language work. Keeping them behind a small API contains their change cost; it
does not remove the need to implement and test them. Prototype package resolution
and the semantic oracle early enough to discover feasibility problems before
building broad CLI functionality.

The compatibility limits above, especially ambient standard libraries and dynamic
module constructs, must remain visible. If real target projects require those
inputs, extend the project/binding contracts and fixtures before advertising
support for them. The architecture accommodates those additions without adding
a second query path or changing the syntax parser into a type checker.

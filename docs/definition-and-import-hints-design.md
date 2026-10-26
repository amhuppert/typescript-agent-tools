# Definitions and import hints: technical design

Status: proposed, 2026-10-26, from tsparse friction a Codex agent reported in
remote-ai-manager ("Improve Docs Tab Features"). Reviewed the same day
against the tree at commit `c91c68e` and a release build of that tree (the
binary predates the commit, and `cargo build --release` reported it
current): every statement below about current behavior cites the code it
was checked against. Every TypeScript answer in [cases](#cases) and
[out of scope](#out-of-scope-positions) was taken from the TypeScript 7.0.2
native compiler's language server, on the source and positions recorded in
[the probe](#the-probe); TypeScript's code is cited from the pinned
typescript-go commit and from TypeScript 5.9.3's compiled source. The user selected both parts:
a **go-to-definition operation**, `--definition`, and a **hint** when
`--show --name` misses a name that the file imports or re-exports. A related
case is already handled: `--at` on an `import` or `export` keyword selects
the whole declaration as a statement site
([statement sites](statement-sites-and-test-view-design.md)). The behavior
contract, once implemented, lives in [references](references.md) (the
operation and its differences from TypeScript) and
[navigation](navigation.md) (the hint).

## Operating envelope

The same as references: one local user, a one-process-per-request CLI with
the optional per-user project service on macOS and Linux, TypeScript 7.0.2
semantics, and one explicit `tsconfig.json` project. `--definition` exists
only in project mode; the hint also appears in file mode. The callers are
agents reading code. They need to go from a file to where a name it uses is
declared: an imported hook, a re-exported route handler, a local helper.
Nothing here persists state, runs concurrently beyond what the service
already handles, or crosses a trust boundary.

## Problem

Reproduced on remote-ai-manager:

- `--show --name useProjectFilesQuery RegisterDocumentDialog.tsx` prints
  ``…/RegisterDocumentDialog.tsx: no declaration matches `useProjectFilesQuery` ``
  and exits 1. The file imports the name with
  `import { useProjectFilesQuery } from "…"`. A non-renaming specifier
  declares no name, so its catalog entry has no `--name` key, as TypeScript's
  workspace symbols count no declaration there
  (`src/navigation/projection.rs:788-805`, `alias_naming`).
- `src/app/api/config/route.ts` is `export { GET, PUT } from
  "@/lib/config/route-handlers";`, and `--show --name GET` on it misses the
  same way.
- In both cases the agent fell back to `rg` to find the module the name came
  from and the declaration inside it.

tsparse can already answer the question, indirectly. `--references --at` on
the specifier follows import and export aliases, and its group header names
the original declaration (`group 1: formatTitle, declared at FILE:LINE:COLUMN`,
`src/cli/references.rs:173-194`). But nothing points an agent there, and a
reference search is the wrong question when the agent only wants the
declaration.

## What exists today

These premises were checked against the code; the design relies on each.

- **One lexical selection, used by `--references`.** A position selects the
  identifier occurrence containing it (`Index::occurrence_at`,
  `src/project/binding/index.rs:300-308`); a name selects every candidate
  spelled exactly NAME: declaration names, local import names, and exported
  alias names (`Index::candidates`, `index.rs:352-355`; candidates are marked
  in `src/project/binding/collect.rs:893-913`, `:2479`, `:2496`, `:2518`,
  `:2578-2583`, `:2629`, `:2658`), with FILE restricting the candidates
  (`src/project/references.rs:161-184`). `selected` turns an occurrence into
  its symbols, or fails with `UnsupportedTarget` (``"`NAME` is not a supported
  reference target (REASON)"``) or `UnresolvedTarget` (``"`NAME`: REASON"``)
  (`references.rs:220-249`). `lexical_symbols_at` and `has_lexical_target`
  (`references.rs:16-46`) are thin views of the same occurrence lookup.
- **Aliases are resolved at load.** Linking forces each import and export
  alias token's resolution while it freezes the index, so an import
  specifier, a re-export, or a use already denotes the final declared symbols
  (`src/project/binding/link.rs:3513-3517`). A specifier's property name
  (`a` in `{ a as b }`) is a token of the same alias (`collect.rs:2517`,
  `:2657`). A string-literal specifier name is no token (`collect.rs:2443-2449`).
- **A symbol's declarations are `Index::sites`.** Each is a name token or a
  synthetic anchor, ordered by file display name and then position, and
  deduplicated (`index.rs:400-402`, `:627-639`), including the sites an
  alias merges into the symbol (`link.rs:3580-3606`, `projected_sites`). The
  reference group's `declarations` are exactly these (`references.rs:251-259`),
  so they need no occurrence search.
- **`--references --at` dispatches between two questions.** The reference
  selection owner (`src/project/semantic/selection/references.rs:35-89`)
  sends a bare `--name` to the lexical question, and a position to it unless
  the token has no lexical target and is a member site, in which case it runs
  a member search (`at`, `:158-193`; member sites are property-access names,
  member declaration names, enum members, private names, JSX attributes,
  binding property names, literal keys, and parameter-property modifiers:
  `src/project/semantic/source/reference_search.rs:408-445`). Reference
  tokens are what `reference_tokens::capture` inventories
  (`src/project/binding/reference_tokens.rs:195-360`): every identifier and
  private name, and other tokens in reference positions. Among the others
  are the modifier that anchors a parameter property, such as `public` in
  `constructor(public x: number)` (`:210-238`); a JSX namespaced attribute
  name such as `xlink:href`, captured as one token spanning both names and
  the colon (`:239-262`); and a string, numeric, or template literal in a
  reference position (`:264-334`). The modifier, the namespaced attribute,
  and a literal that names or keys a member (`"q"` in `{ "q": 2 }`,
  `obj["q"]`) are member sites (`reference_search.rs:416-444`); another
  literal reference token, such as the argument of `import("./m")` in a
  type, is not. So on `page.title`, `public`, or `"q"`, `--references --at`
  answers with `coverage member` and exits 0. Selecting the member can
  itself fail: a JSX attribute is `UnsupportedSemantic` with reason
  `jsx_typing_required`
  (`src/project/semantic/evaluation/reference_symbols.rs:1630-1641`, exit 1),
  and on any character of `xlink:href`, its colon included, selecting the
  member fails today with `Internal` (`canonical semantic declaration
  association is not retained`, raised at `src/project/semantic/mod.rs:284`
  or `:322`, exit 2), with or without JSX types declared, before any search
  starts ([the probe](#the-probe)); both reproduced. At a position in no reference
  token and no identifier occurrence (whitespace, a comment, punctuation
  outside a captured token, a keyword other than a parameter property's
  modifier), and at a string literal that is neither a member name or key
  nor a lexical name (a module specifier, a string specifier name, an
  `import("./m")` argument), it answers 0 groups and exits 0
  (`docs/references.md:508`). Labels, `import.meta`, `this` in a type query,
  and the like fail with `UnsupportedTarget` (`docs/references.md:508-512`).
- **Lexical member access stops at namespaces.** `ns.f` through a namespace
  import, a namespace, or `import x = require()` resolves lexically to the
  export; a property of a value (`obj.method`) and an enum member resolve to
  `NotReference("property")` or `("enum member")`
  (`src/project/binding/resolution.rs:1251-1323`).
- **Requests run in a per-request semantic worker.** `--references` creates a
  fresh checker per request, initializes it, then selects
  (`src/project/semantic/mod.rs:53-66`, `docs/types.md:14`,
  `docs/references.md:424`). On a warm service the input check dominates:
  about 0.1 s of a 0.12–0.16 s query on a 4,154-file project
  (`docs/references.md:101-105`).
- **An unresolved module fails the load.** A missing module in a file
  TypeScript checks is `UnresolvedImport` for every operation (exit 1); in a
  file it skips, such as one under `// @ts-nocheck`, the names that depend
  on it are `UnresolvedTarget` (`docs/references.md:541-546`, `:1095-1101`).
  Both were reproduced.
- **The navigation catalog holds unkeyed specifier entries; the captured
  index does not.** A complete catalog has one `import` entry per default
  import, namespace import, import specifier, and `import x =`, and one
  `export` entry per export specifier and `export =`, each anchored at its
  name, keyed only where TypeScript's workspace symbols count a declaration
  (`projection.rs:432-441`, `:606-663`, `:788-805`; `docs/navigation.md:233-245`).
  `export * as ns` and `export *` have no entry (`projection.rs:466-473`
  visits the clause, and a `NamespaceExport` is no site). The snapshot keeps
  only keyed sites (`src/navigation/catalog.rs:285-292`); unkeyed entries come
  from projecting the file's captured text again, as `--show --at` does
  (`src/project/navigation.rs:266-303`).
- **Text names files absolutely; JSON relative to the configuration.**
  Result text joins the configuration's directory with each file
  (`src/cli/navigation.rs:195-215`, `place`), and headers print the loaded
  configuration's canonical path; JSON names files relative to its
  directory (`src/cli/references.rs:7-12`).
- **Printed follow-up commands.** `command_line::render` is the one renderer
  (`src/cli/command_line.rs:5-25`). A document carries the command as
  `next: {"argv": ["tsparse", …]}` (`src/cli/failure.rs:283-284`, `:413-421`).
  A printed command keeps the request's own spelling of its arguments and
  names no transport: it leaves out `--direct`, so the service's and a
  direct answer stay byte-identical (`src/cli/request.rs:349-412`). A command
  that cannot be completed is written with a placeholder, as ambiguity lines
  write ``select one with --at FILE:LINE:COLUMN`` and no `next`
  (`src/cli/navigation.rs:307-312`, `src/cli/references.rs:336-342`);
  `TSCONFIG` is `--project`'s registered placeholder
  (`src/cli/registry.rs:212-216`). Every advertised remedy is executed by a
  test before it ships (`docs/agent-cli-design.md:547`).
- **Plumbing per operation.** A project operation is a `ProjectRequest` and
  `ProjectResponse` variant (`src/project/mod.rs:280-293`, `:386-398`), an
  `execute_unless` arm (`:186-205`), an `operands()` arm (`:312-355`), a
  registry entry, a `Target::operation` name (`src/cli/request.rs:162-204`),
  a request builder (`:663-822`), and a delivery arm
  (`src/cli/query.rs:152-180`). The service has no per-operation code.

## TypeScript's definition answer

TypeScript answers a definition request with `getDefinitionAtPosition`
(TypeScript 5.9.3's compiled `typescript.js`, line 174321; TypeScript 7.0.2
ports it to Go). In order, it answers file references (`/// <reference>`
directives and relative module specifiers, 174323 and 174511-174546),
`override` (174334), labels (174340), `case` and `default`
(174352-174362), `return`, `await`, and `yield` (174366), and `static` of a
class static block (174373). Otherwise it takes the symbol at the token
(`getSymbol`, 174395 and 174677-174689): an alias is replaced by its final
target (`getAliasedSymbol`) when `shouldSkipAlias` allows it (174690-174700);
when the target has no declarations the alias itself is kept, marked
`failedAliasResolution` (174685). A module specifier with no symbol gives the
resolved file (174405). At a call or `new` target it adds the called
signature (174428-174438). Each declaration is reported at its name, or,
for a declaration without one, at the whole node (174768-174771); an
`export default` or `export =` of an expression other than a name has none
(`getNonAssignedNameOfDeclaration`, 15085-15088), nor has an anonymous
`export default function`.

TypeScript 7.0.2, the baseline, is `internal/ls/definition.go` at the pinned
typescript-go commit
([`2bd066d`](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/ls/definition.go)).
Its `provideDefinitionWorker` keeps the `override`, label, `case`/`default`,
and `return`/`yield`/`await` branches and has no class-static-block branch.
`getDeclarationsFromLocation`
([L230-L300](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/ls/definition.go#L230-L300))
answers a shorthand property name with the declarations of
`GetResolvedSymbol(node)`, taken before any alias resolution, followed by
the contextual type's property declarations (L230-L244).
`GetResolvedSymbol` wraps `getResolvedSymbol`
([checker.go L13814-L13826](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/checker.go#L13814-L13826)),
which resolves the name through `resolveName`, whose symbol-table lookup
`getSymbol`
([L2162-L2174](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/checker.go#L2162-L2174))
returns an alias unresolved when its target has the requested meaning; so a
shorthand of an imported name answers the import specifier. It answers
the name of an object binding element that is not a rest element (no `...`)
with the matching property declarations of the pattern's type (L247-L274).
Otherwise it maps a keyword that precedes a declaration's name to that name
(`getDeclarationNameForKeyword`), takes `GetSymbolAtLocation`, replaces a
class symbol at its `constructor` keyword with the constructor, always
resolves an alias that resolves (`ResolveAlias`), and returns the symbol's
declarations (L276-L300). Its call branch
([L78-L96](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/ls/definition.go#L78-L96))
asks whether the symbol at the location, before alias resolution, matches the
called signature: if so it answers the called declaration alone, or, at a
constructor, the class declarations and the constructor, as 5.9.3 does
(174433, 174434); if not, it keeps every declaration and appends the called
one. For a callee reached through an import alias the symbol at the location
is the alias, which never matches. So 7.0.2 answers these cases differently
from the 5.9.3 source:

- At a call of an overloaded function reached through an import alias,
  5.9.3 returns only the resolved overload (174433); 7.0.2 returns every
  declaration, the resolved one last. A local or namespace-qualified call
  (`over(1)` in the declaring file, `lib.over(3)`) returns only the resolved
  overload in both.
- At `new C()` reached through an import alias, with a constructor and a
  merged interface, 5.9.3 keeps the class declarations and the constructor
  (174434); 7.0.2 also keeps the interface. A local or namespace-qualified
  `new` keeps the class declarations and the constructor in both.
- On an overloaded function's own name, 5.9.3 returns only the declarations
  with a body (174739, 174750); 7.0.2 returns every declaration.
- On a use of a namespace import (`lib` in `lib.f`), 5.9.3 keeps the alias,
  so it returns the `* as lib` specifier (174697); 7.0.2 returns the module.
- On the `static` of a class static block, 5.9.3 returns the static blocks
  (174373); 7.0.2 returns nothing.

### Cases

Each row gives the probe's positions ([the probe](#the-probe)), TypeScript
7.0.2's answer there, and this design's: today's `--references --at` group
header at the same positions, which `--definition` prints. Locations are
`FILE:LINE:COLUMN` in the probe's `src/`, written with the file once when
several share it; a TypeScript range is shown where the span is more than a
name. "Same" means the same declarations at the same locations.

| # | Case | Positions | TypeScript 7.0.2 | This design |
| --- | --- | --- | --- | --- |
| 1 | A use of a name imported through the `index.ts` barrel, called and not called | `app.ts:12:11`, `app.ts:13:11` | `lib.ts:2:17` | same |
| 2 | The local name of `import { formatTitle } from "./index"` | `app.ts:1:10` | `lib.ts:2:17` | same |
| 3 | `import { formatTitle as fmt }`: the property name, the local name, a use of `fmt` | `app.ts:2:10`, `app.ts:2:25`, `app.ts:14:11` | `lib.ts:2:17` | same |
| 4 | A default import of `export default function makeThing() {}` (its name and a call) and of `export default formatTitle` | `app.ts:3:8`, `app.ts:17:1`; `app.ts:10:8` | `def.ts:1:25`; `lib.ts:2:17` | same |
| 5 | A default import of `export default 42;` (its name and a use) and of `export default function () {}` | `app.ts:4:8`, `app.ts:21:18`; `app.ts:5:8` | the statement, `anon.ts:1:1-1:19`; `anonfn.ts:1:1-1:30` | `default` at `anon.ts:1:8`; `anonfn.ts:1:8`: **K2** |
| 6 | A namespace import `* as lib`: its name and a use | `app.ts:6:13`, `app.ts:18:1` | the file from its first token, `lib.ts:2:1-18:1` (line 1 is a comment) | the module `src/lib.ts` at `lib.ts:1:1`: **K2** |
| 7 | `formatTitle` in `lib.formatTitle("x")` | `app.ts:18:5` | `lib.ts:2:17` | same |
| 8 | `import eq = require("./eq")`, whose module ends `export = config`: its name and a use | `app.ts:7:8`, `app.ts:21:37` | `eq.ts:1:7` | same |
| 9 | `import eqObj = require("./eqobj")`, whose module ends `export = { a: 1 };` | `app.ts:8:8` | the statement, `eqobj.ts:2:1-2:19` | `export=` at `eqobj.ts:2:8`: **K2** |
| 10 | `export { formatTitle } from "./lib"`; a local `export { formatTitle }` of an import; both names of `export { formatTitle as renamed } from "./lib"`; an import of `renamed` | `index.ts:1:10`; `barrel.ts:2:10`; `barrel.ts:5:10`, `barrel.ts:5:25`; `app.ts:9:16` | `lib.ts:2:17` | same |
| 11 | `deepValue`, imported from `index.ts`, which reaches `deep.ts` through two `export *` declarations: the import and a use | `app.ts:1:23`, `app.ts:21:25` | `deep.ts:1:14` | same |
| 12 | `export * as star from "./lib"`, and an import of `star` | `barrel.ts:3:13`, `app.ts:9:10` | `lib.ts:2:1-18:1` | `lib.ts:1:1`: **K2** |
| 13 | `config` in `export = config` | `eq.ts:2:10` | `eq.ts:1:7` | same |
| 14 | `import { missing } from "./missing-module"`, unresolved: the name and a use; an unresolved import in a `// @ts-nocheck` file | `missing.ts:1:10`, `missing.ts:2:18`; `nocheck.ts:3:19` | the specifier itself, `missing.ts:1:10-1:17`; `nocheck.ts:2:10-2:14` | the load fails, `src/missing.ts:1:25: cannot resolve module "./missing-module"` (`UnresolvedImport`, exit 1); without `missing.ts`, ``nocheck.ts:3:19: `gone`: module is not resolved`` (`UnresolvedTarget`, exit 1): **K3** |
| 15 | A declaration's own name: a function; an overload or the implementation; a class merged with an interface | `lib.ts:2:17`; `lib.ts:5:17`, `lib.ts:7:17`; `lib.ts:10:14` | itself; `lib.ts:5:17`, `6:17`, `7:17`; `lib.ts:10:14`, `13:18` | same |
| 16a | A call of an overloaded function reached through an import alias, `over("s")` | `app.ts:15:1` | `lib.ts:6:17`, `7:17`, `5:17`: every declaration, the resolved overload last | `lib.ts:5:17`, `6:17`, `7:17`: **K1** |
| 16b | A local call, `over(1)` in `lib.ts`, and a namespace-qualified one, `lib.over(3)` | `lib.ts:16:1`, `app.ts:19:5` | the resolved overload alone, `lib.ts:6:17` | `lib.ts:5:17`, `6:17`, `7:17`: **K1** |
| 17a | `new Box(2)` reached through an import alias; `Box` has a constructor and a merged interface | `app.ts:16:5` | `lib.ts:10:14`, `13:18`, and the constructor `lib.ts:11:3-11:38` | `lib.ts:10:14`, `13:18`: **K1** |
| 17b | A local `new Box(1)` and a namespace-qualified `new lib.Box(4)` | `lib.ts:17:5`, `app.ts:20:9` | `lib.ts:10:14` and the constructor `lib.ts:11:3-11:38`, without the interface | `lib.ts:10:14`, `13:18`: **K1** |
| 18 | The name of `const { a } = source`, an object binding element without a property name; a use of `a` | `bind.ts:2:9`; `bind.ts:6:21` | the property `a` of `source`, `bind.ts:1:18`; `bind.ts:2:9` | the binding, `bind.ts:2:9`: **K8**; same |
| 19 | The shorthand `{ b }` in `const value: Shape = { b }` | `bind.ts:5:31` | the variable, `bind.ts:4:7`, then the contextual property `Shape.b`, `bind.ts:3:19` | the variable, `bind.ts:4:7`: **K8** |
| 20 | The shorthand `{ formatTitle }` of an imported name, `const obj = { formatTitle }` | `short.ts:4:22` | the import specifier, `short.ts:1:10-1:21` | the declaration the import resolves to, `lib.ts:2:17`: **K8** |
| 21 | The name of `const { ...rest } = source`, a rest element, and a use of `rest` | `short.ts:3:12`, `short.ts:5:20` | the binding, `short.ts:3:12` | same |

### Out-of-scope positions

Positions off the identifier path: members, keywords, labels, and string
literals, which this design leaves out of scope. Its answer follows from
[selection](#selection): a member position gives the `member` outcome, or
the failure that selecting the member reports; a position that selects
nothing gives `not_found`; and a refused token gives its error. Today's
`--references --at` at the same position shows the dispatch: `coverage
member`, 0 groups, or the error, which a failure while selecting a member
is for both operations.

| Case | Position | TypeScript 7.0.2 | `--references --at` today | This design |
| --- | --- | --- | --- | --- |
| A property of a value, `eq.a` | `app.ts:21:40` | `eq.ts:1:18` | `coverage member` | `member`: K4 |
| A property access on `this`, `this.n` | `kw.ts:17:21` | the parameter property, `kw.ts:2:22` | `coverage member` | `member`: K4 |
| A member declaration's name, `run` | `kw.ts:11:3` | itself | `coverage member` | `member`: K4 |
| A parameter property's modifier, `public` | `kw.ts:2:15` | the parameter, `kw.ts:2:22` | `coverage member` | `member`: K4 |
| A string literal member name, `"q"` in `{ n: 1, "q": 2 }` | `kw.ts:22:35` | itself, `kw.ts:22:34` | `coverage member` | `member`: K4 |
| A literal element access key, `"a"` in `source["a"]` | `short.ts:7:28` | the property, `short.ts:2:18` | `coverage member` | `member`: K4 |
| A JSX attribute, `label` in `<Item label="x" />` | `jsx.tsx:4:27` | the props member, `jsx.tsx:1:24` | ``jsx.tsx:4:27: a JSX attribute relates to its props member only through deferred JSX attribute typing`` (`UnsupportedSemantic`, exit 1) | the same failure: K4 |
| A JSX namespaced attribute, `xlink:href`: its namespace, its colon, and its name | `jsx.tsx:5:26`, `jsx.tsx:5:31`, `jsx.tsx:5:32` | nothing | `canonical semantic declaration association is not retained` (`Internal`, exit 2), raised while selecting | the same failure: K4 |
| `super` in `super(1)` | `kw.ts:6:5` | the base class, `kw.ts:1:7`, and its constructor, `kw.ts:2:3-2:35` | 0 groups | `not_found`: K1, K5 |
| `this` | `kw.ts:17:16` | the class, `kw.ts:4:7` | 0 groups | `not_found`: K5 |
| `return` | `kw.ts:17:9` | the method, `kw.ts:11:3` | 0 groups | `not_found`: K5 |
| `case` | `kw.ts:16:7` | the `switch`, `kw.ts:15:5` | 0 groups | `not_found`: K5 |
| The label in `break outer` | `kw.ts:13:13` | the label, `kw.ts:12:5` | ``kw.ts:13:13: `outer` is not a supported reference target (label)`` | the same `UnsupportedTarget`: K5 |
| `constructor` in `Base` | `kw.ts:2:3` | the constructor, `kw.ts:2:3-2:35` | 0 groups | `not_found`: K5 |
| `static` of a class static block | `kw.ts:8:3` | nothing | 0 groups | `not_found`, the same answer |
| `function` and `export` before `named` | `kw.ts:23:8`, `kw.ts:23:1` | the name, `kw.ts:23:17` | 0 groups | `not_found`: K5 |
| The `import` of `import eq = require("./eq")` | `app.ts:7:1` | `eq.ts:1:7` | 0 groups | `not_found`: K5 |
| The `import` of an import declaration | `app.ts:1:1` | nothing | 0 groups | `not_found`, the same answer |
| A module specifier, `"./index"` | `app.ts:1:41` | the file, `index.ts:1:1` | 0 groups | `not_found`: K6 |
| The argument of `typeof import("./lib")` in a type | `short.ts:6:32` | the file, `lib.ts:2:1-18:1` | 0 groups | `not_found`: K6 |
| A string specifier name, `"f-g"` in `export { "f-g" } from "./strings"` | `kw.ts:24:11` | `fg`, `strings.ts:1:7` | 0 groups | `not_found`: K6 |

### Known differences

This design returns, for an identifier, the declarations of the symbols it
denotes after following import and export aliases. Every other TypeScript
definition case is out of scope, and each difference is recorded in
[references](references.md) when implemented.

- **K1. Calls and `new`.** TypeScript adds the called signature. Where the
  callee's symbol at the location matches the called signature, as at a
  local or namespace-qualified call or `new` (rows 16b, 17b), it answers the
  resolved overload alone, or the class declarations and the constructor
  without a merged interface, as 5.9.3 does. Where the callee is reached
  through an import alias (rows 16a, 17a), it keeps every declaration and
  appends the called one: every overload with the resolved one last, or the
  class, the interface, and the constructor. At `super(…)` it answers the
  base class and its constructor. This design never selects a signature: at
  every call and `new` it lists the symbol's declarations in location order,
  and never a constructor.
- **K2. Locations of declarations without a name.** An anonymous default
  export, `export =` of an expression other than a name, and a module used as
  a namespace keep the reference model's anchors: `default` at the `default`
  keyword, `export=` at the `=`, and the module's path at `1:1`
  (`docs/references.md:494-499`). TypeScript starts each at the declaration's
  first token, and a file's at its first token after leading trivia. The
  declaration is the same; only its location differs.
- **K3. Unresolved modules.** TypeScript returns the import specifier itself;
  here the load fails with `UnresolvedImport` (exit 1), or, in a file
  TypeScript skips, the name is `UnresolvedTarget` (exit 1).
- **K4. Members.** TypeScript follows a property of a value (`obj.method`,
  `this.n`), a member declaration's name, an enum member, a private name, a
  JSX attribute, a binding element's property name (`a` in `{ a: x }`), an
  object literal key, a string or numeric literal member name or key (`"q"`,
  `obj["q"]`), a parameter property's modifier (`public` answers the
  parameter), and an index signature. Here every position the reference
  selection sends to its member path gets the `member` outcome
  ([member positions](#member-positions)), or the failure that selecting the
  member reports, such as `UnsupportedSemantic` for a JSX attribute and,
  today, `Internal` (exit 2) for a JSX namespaced attribute, where
  TypeScript 7.0.2 answers nothing. A
  qualified member in a type position (`let x: E.A`, `typeof value.prop`) is
  not a member site for that dispatch, and is `UnsupportedTarget`, as
  `--references --at` reports it.
- **K5. Keywords and labels.** TypeScript answers `this` (the class), `super`,
  `return`, `await`, and `yield` (the function), `case` and `default` (the
  `switch`), `override` (the overridden member), and labels; and it maps a
  keyword that precedes a declaration's name to that name (`function`,
  `class`, `const`, an `export` before a declaration, the `import` of
  `import x = require()`), and a class's `constructor` keyword to its
  constructor. Here a keyword is no identifier, so it is a
  missing selection, unless it is a parameter property's modifier, which is a
  member position (K4); a label is `UnsupportedTarget` (`label`). Neither
  compiler answers the `import` of an import declaration, and 7.0.2 does not
  answer the `static` of a class static block (5.9.3 returns the static
  blocks), so `not_found` there is the same answer.
- **K6. File references and string names.** TypeScript answers a module
  specifier string with its file, the argument of a type's `import("./m")`
  with its file, a `/// <reference>` with its target, and a string-literal
  specifier name (`export { "f-g" } from`) with its target. None is an
  identifier or a member name or key here, so each is a missing selection.
- **K7. The lexical model's differences.** Every documented difference of the
  lexical model applies unchanged, such as a name that two `export *`
  declarations supply with different targets being unresolved
  (`docs/references.md:955`).
- **K8. Shorthand names and binding elements.** At the name of an object
  binding element that has no property name and is not a rest element
  (`const { a } = source`, row 18), TypeScript answers the matching property
  declarations of the pattern's type; a rest element's name
  (`const { ...rest } = source`, row 21) answers its own declaration, as here.
  At a shorthand property name, TypeScript answers the declarations of the
  name's resolved symbol without resolving an alias, then the contextual
  type's property declarations (`getDeclarationsFromLocation`). So with a
  contextual type the answer adds that type's property (`{ b }` typed
  `Shape`, row 19), and for an imported name its declaration is the import
  specifier, not the declaration the import names (`{ formatTitle }`, row
  20). Here the binding element's
  name declares the binding (`src/project/binding/collect.rs:1304-1311`) and
  the shorthand name is a use of the value (`collect.rs:1638-1644`), so the
  answer is the binding, or the value's declarations alone, and an imported
  shorthand name follows its alias to the original declaration, as every
  other use of the import does (row 1). That is deliberate: the alias is
  what `--definition` exists to follow. A use of such a binding is the same
  in both.

### The probe

The answers above were taken from TypeScript 7.0.2's native compiler
(`@typescript/typescript-linux-x64` 7.0.2; `tsc --version` prints
`Version 7.0.2`), with `textDocument/definition` requests sent through the
language-server client in `tools/navigation-oracle.mjs`
(`DocumentSymbolServer`), and from this build's
`--references --direct --at --json` at the same positions, one-based lines
and UTF-16 columns. This build answered on the same files without
`src/missing.ts`, whose unresolved import fails the load (row 14); with it,
every query fails with that load error. Rows 16 and 17 have three variants
because TypeScript's answer depends on whether the callee is reached through
an import alias; the tests cover each. `short.ts`, `jsx.tsx`, and the `jsx`
option were added for rows 20 and 21 and the literal-key, `import("./m")`,
and JSX positions; every other row was queried again on the extended project
and gave the same answers. Whether a member position's failure arises while
the member is selected or later, in the search, cannot be read from the code
alone: `TypeBindingFailure::NotRetained`, the source of `canonical semantic
declaration association is not retained` (`src/project/semantic/mod.rs:284`,
`:322`), has producers in `src/project/semantic/evaluation/names.rs:347` and
throughout `src/project/binding/native_inputs.rs` (`:207` to `:759`), on
paths that selection, through `reference_symbol_at_node`
(`semantic/evaluation/reference_symbols.rs:1642`, `:1840-1847`), and the
search can both reach. It was settled with a scratch copy of this tree,
outside the worktree, that prints one line once
`selection::references::at` has selected a member: the line appears at
`source["a"]` and `public` before their member searches, and not at any
character of `xlink:href`, whose `Internal` failure therefore comes from
selecting. The implementation's tests take their expected outcomes from these
TypeScript answers, not from tsparse's output.

`tsconfig.json`:

```json
{
  "compilerOptions": {
    "strict": true,
    "target": "es2022",
    "module": "nodenext",
    "moduleResolution": "nodenext",
    "jsx": "preserve"
  },
  "include": [
    "src"
  ]
}
```

`src/lib.ts`:

```ts
// leading comment
export function formatTitle(title: string): string {
  return title.trim();
}
export function over(a: string): string;
export function over(a: number): number;
export function over(a: any): any {
  return a;
}
export class Box {
  constructor(public size: number) {}
}
export interface Box {
  label?: string;
}
over(1);
new Box(1);
```

`src/def.ts`:

```ts
export default function makeThing() {}
```

`src/anon.ts`:

```ts
export default 42;
```

`src/anonfn.ts`:

```ts
export default function () {}
```

`src/deep.ts`:

```ts
export const deepValue = 1;
```

`src/more.ts`:

```ts
export * from "./deep";
```

`src/index.ts`:

```ts
export { formatTitle } from "./lib";
export * from "./more";
```

`src/eq.ts`:

```ts
const config = { a: 1 };
export = config;
```

`src/eqobj.ts`:

```ts
// leading comment
export = { a: 1 };
```

`src/barrel.ts`:

```ts
import { formatTitle } from "./lib";
export { formatTitle };
export * as star from "./lib";
export default formatTitle;
export { formatTitle as renamed } from "./lib";
```

`src/app.ts`:

```ts
import { formatTitle, deepValue } from "./index";
import { formatTitle as fmt, over, Box } from "./lib";
import makeThing from "./def";
import anon from "./anon";
import anonFn from "./anonfn";
import * as lib from "./lib";
import eq = require("./eq");
import eqObj = require("./eqobj");
import { star, renamed } from "./barrel";
import dflt from "./barrel";

const h = formatTitle(" Docs ");
const f = formatTitle;
const g = fmt;
over("s");
new Box(2);
makeThing();
lib.formatTitle("x");
lib.over(3);
new lib.Box(4);
export const n = anon + deepValue + eq.a;
export const values = [anonFn, eqObj, star, renamed, dflt, h, f, g];
```

`src/bind.ts`:

```ts
const source = { a: 1 };
const { a } = source;
interface Shape { b: number }
const b = 1;
export const value: Shape = { b };
export const used = a;
```

`src/short.ts`:

```ts
import { formatTitle } from "./lib";
const source = { a: 1, b: 2 };
const { ...rest } = source;
export const obj = { formatTitle };
export const all = rest;
export type T = typeof import("./lib").formatTitle;
export const key = source["a"];
```

`src/kw.ts`:

```ts
class Base {
  constructor(public n: number) {}
}
class Child extends Base {
  constructor() {
    super(1);
  }
  static {
    void 0;
  }
  run(x: number): number {
    outer: for (;;) {
      break outer;
    }
    switch (x) {
      case 1:
        return this.n;
    }
    return 0;
  }
}
export const shorthand = { n: 1, "q": 2 };
export function named() {}
export { "f-g" } from "./strings";
```

`src/strings.ts`:

```ts
const fg = 1;
export { fg as "f-g" };
```

`src/jsx.tsx`:

```tsx
function Item(props: { label: string }) {
  return null as any;
}
export const item = <Item label="x" />;
export const link = <svg xlink:href="#x" />;
```

`src/missing.ts`:

```ts
import { missing } from "./missing-module";
export const m = missing;
```

`src/nocheck.ts`:

```ts
// @ts-nocheck
import { gone } from "./gone-module";
export const g2 = gone;
```

### Conformance

The semantic oracle (`tools/semantic-oracle.mjs`) classifies every identifier
by `getSymbolAtLocation` followed by `getAliasedSymbol`, a shorthand property
name by its value's symbol (`:17-25`, `:358-363`, `:385-418`), normalizes
declarations to the anchors of K2 (`:78-82`), and
`tools/semantic-conformance.mjs compare` checks that a position query at
every such token returns those declaration sets (`:17-27`;
`examples/reference_batch.rs:132` calls `Project::find_references`). So the
declaration sets this operation prints are already checked against the
pinned compiler at every identifier, within the lexical model's documented
differences. The oracle never asks for TypeScript's definition answer, so
K1, K3, K8, and TypeScript's own locations are outside it; they were checked
with the probe, and the tests pin them.

## Go to definition

### Command line

```text
tsparse --definition --project TSCONFIG (--at LINE:COLUMN FILE | --at FILE:LINE:COLUMN | --name NAME [FILE]) [--direct] [--json [--pretty]] [--output OUTPUT]
```

The registry entry follows `--type`'s (`src/cli/registry.rs:864-937`):

- **Flag** `--definition`, a switch: "Find the declarations a name denotes,
  following import and export aliases, in one project."
- **Accepted flags** `--project`, `--direct`, `--at`, `--name`, `--json`,
  `--pretty`, `--output` (`-o`). `--pretty` needs `--json`
  (`Pretty::WithJson`). A value flag given twice is a usage error
  (`RepeatedValue::Reject`). It bounds no list, so it takes no `--limit`.
- **Related** `--references`, `--show`; **document** `docs/references.md`.
- **Examples** `--definition --project tsconfig.json --at src/app.ts:1:10`
  ("The declaration an import resolves to, through re-exports.") and
  `--definition --project tsconfig.json --name formatTitle src/app.ts` ("The
  declarations of a name src/app.ts declares, imports, or exports.").

Usage errors are decided before anything starts or loads, each exit 2. All
but the dotted-name refusal, which is new, are the messages the existing
builders already give `--references` and `--type` (`src/cli/request.rs:568-605`,
`:729-744`, `:778-799`, `:1108-1127`):

| Request | Code and message |
| --- | --- |
| No `--project` | `usage.missing_flag`: `--definition requires --project TSCONFIG` |
| Both or neither of `--at` and `--name` | `usage.selection`: `--definition requires exactly one of --at or --name` |
| `--at LINE:COLUMN` without FILE | `usage.operands`: `--definition --at needs a FILE operand or FILE:LINE:COLUMN` |
| `--at FILE:LINE:COLUMN` beside FILE | `usage.operands`: `--at FILE:LINE:COLUMN names the file; omit the FILE operand` |
| A malformed `--at` | `usage.invalid_value`: `--at expects LINE:COLUMN or FILE:LINE:COLUMN, got VALUE` |
| FILE `-` | `usage.operands`: `--definition reads files from the project, not stdin` |
| Two FILE operands | `usage.operands`: `Expected one input file` |
| A `--name` containing `.` | `usage.invalid_value`: ``--definition --name takes one identifier, not NAME: a dotted name selects a member by containment, which --definition does not follow``, with `next`: `tsparse --show --project TSCONFIG --name NAME`, TSCONFIG as the request spelled it |
| `--pretty` without `--json` | `usage.flag_scope`: `--pretty applies to --definition only with --json` |
| A value flag twice | `usage.repeated_flag`: `--definition takes --at at most once` (or the flag given) |
| A flag it does not accept (`--limit`, `--calls`, `--file`, `--depth`, …) | `usage.flag_scope`: `FLAG applies to: OPERATIONS` |
| Another operation's flag | `usage.operation`: `Choose one operation: FLAGS` |

The dotted-name `next` drops FILE: `--show`'s FILE narrows to declarations in
FILE, while `--definition`'s names the file where the name is spelled. With
`--json`, a usage failure is the usual error document on stdout
(`{"error":{"category":"usage","code":…,"message":…},"operation":"definition","outcome":"error"}`,
with `next` inside `error` where it has one).

### Selection

The library entry is `Project::definition(ReferenceQuery)`, also
`ProjectRequest::Definition(ReferenceQuery)`. It runs in the semantic worker
`--references` uses, initializes its checker as `semantic::references` does,
and asks the reference selection owner (`selection::references::select`):

- **`--at FILE:LINE:COLUMN`.** FILE must be one of the searched files
  (`OutsideProject`, exit 2) and the position inside it (`InvalidPosition`,
  exit 2). Then `select` decides in this order (`at`,
  `selection/references.rs:158-193`), so every position has one outcome:
  1. If a reference token contains the position, the identifier occurrence
     there (if any) denotes no symbol, and the token is a member site, the
     position is a **member position**: the member is selected as
     `--references --at` selects it, and the answer is the `member` outcome
     or the failure of that selection ([member positions](#member-positions)).
     A parameter property's modifier and a literal member name or key are
     reference tokens that are no identifiers, and a JSX namespaced attribute
     (`xlink:href`) is one reference token spanning both of its names and
     the colon, whose names denote no symbol; so a position on any of them
     ends here, never in step 2 or 3.
  2. Otherwise the identifier occurrence containing the position decides,
     exactly as for `--references --at`: it gives its symbols, or it is a
     refused token (`UnsupportedTarget`) or an unresolved one
     (`UnresolvedTarget`) (`references.rs:149-160`, `:220-249`). A name with a
     lexical target ends here even where its syntax is member access, as
     `formatTitle` in `lib.formatTitle` does (row 7).
  3. Otherwise nothing is selected, and the answer is `not_found`: a
     position in no reference token and no identifier occurrence (whitespace,
     a comment, punctuation outside a captured token, any other keyword), or
     on a string literal that is neither a member name or key nor a lexical
     name (a module specifier, a string specifier name, an `import("./m")`
     argument).

  A failure while `select` captures the token's site is the answer too,
  whatever step it occurs in ([failures](#failures-and-exit-codes)).
- **`--name NAME [FILE]`.** NAME is one identifier; the library refuses a
  dotted NAME as `InvalidRequest` (exit 2), which the CLI reports first as a
  usage error. Every candidate spelled exactly NAME (decoded, case-sensitive)
  in the searched files, or only in FILE (`OutsideProject` when FILE is not
  searched), gives its symbols; a refused or unresolved candidate fails the
  request with its error at its location, as `--references --name` does. So
  `--definition --project tsconfig.json --name useProjectFilesQuery
  RegisterDocumentDialog.tsx` answers the reported question: the file's
  import specifier is a candidate. A name FILE only uses, without declaring,
  importing, or exporting it, is no candidate; `--at` selects such a use.

The selected symbols are sorted and deduplicated, so two aliases of one symbol
give one group and two shadowed declarations matched by one name give two.
Selection never asks the caller to choose: there is no ambiguous outcome.

### Result

One group per selected symbol, listing that symbol's declarations
(`Index::sites`): the original declarations behind every import and export
alias, plus the sites `projected_sites` merges in. Declarations are in
location order (file as JSON names it, then start, then end); groups are
ordered by their first declaration, as reference groups are
(`references.rs:192`). A group always lists at least one declaration in
practice: even a module that only a pattern ambient module declares gets the
pattern's site. Every group and declaration is printed; nothing is cut, as
reference results never cut groups or declarations
(`src/cli/references.rs:14-17`). A declaration in a dependency or standard
library is printed like any other; such a file is not searched, so
`--show --at` without `--project` reads it.

**Text** (stdout or `-o`, exit 0): a header, then each group and its
declarations. The header is one of

```text
CONFIG: at FILE:LINE:COLUMN: N groups, M declarations
CONFIG: `NAME` N groups, M declarations[; candidates narrowed to FILE]
```

and each group is

```text
group K: NAME
  FILE:LINE:COLUMN[ (OTHER)]
```

- `CONFIG` is the loaded configuration's canonical path. The selection is
  written as reference headers write it (`src/cli/references.rs:141-149`).
  `N` counts groups and `M` declarations across groups, each singular for
  one (`1 group`, `1 declaration`). The narrowing clause appears with
  `--name` and FILE. `K` numbers the groups from 1.
- Each group line names the group by its first declaration's name. Each
  declaration line is its start position; `(OTHER)` follows only when that
  declaration's name differs from the group's, as reference text shows a
  renamed spelling.
- Every FILE is absolute (`place`), so each line is an operand for `--at`
  from any directory. A declaration at a name token is also a `--show --at`
  operand; one at a synthetic anchor (K2) is not, since `--show --at` selects
  catalog entries and statements, and neither is anchored there (`--show
  --at` on `src/anon.ts:1:8` of `export default 42;` is a missing selection).

**JSON** (`--json`, stdout or `-o`, exit 0), fields in this order:

```json
{"operation":"definition","outcome":"complete","project":"/abs/tsconfig.json","selection":{"at":{"file":"src/app.ts","position":{"line":1,"column":10}}},"groups":[{"declarations":[{"name":"formatTitle","location":{"file":"src/lib.ts","start":{"line":1,"column":17},"end":{"line":1,"column":28}}}]}]}
```

`project` and `selection` are as in a reference result: `selection` is
`{"at":{"file":…,"position":{"line":…,"column":…}}}` or
`{"name":{"name":…,"file":…|null}}`, files relative to the configuration's
directory with `/` separators (`references.rs:53-71`). A declaration is a
`NamedLocation`, as in a reference group. `--pretty` indents the same
document.

**Missing selection** (exit 1). When nothing is selected, a position that
reaches step 3 or a name with no candidate, the answer is the `not_found`
outcome, never an empty success: an agent must not read "no groups" as "no
declaration exists". This differs from `--references`, which succeeds with no
groups (its registry exit 0), and follows `--show`, whose missing selection
exits 1 (`docs/navigation.md:157-158`). Text is a diagnostic on stderr, the
first form for `--at` and the second for `--name`:

```text
FILE:LINE:COLUMN: no identifier is at this position; --definition follows identifiers, not keywords or string literals
CONFIG: no declaration, import, or export matches `NAME`[; candidates narrowed to FILE]
```

The document (stdout or `-o`) is
`{"operation":"definition","project":…,"selection":…,"outcome":"not_found"}`.

### Member positions

At a member position (step 1), `select` selects the member as
`--references --at` does before it searches: it resolves the member's
symbol. When that succeeds, the answer is the `member` outcome (exit 1),
naming the search that `--references --at` runs at the same position, whose
group header lists the member's declarations. When it fails, that failure is
the answer instead, delivered as every analysis failure is
([failures](#failures-and-exit-codes)), and no search is named, since
`--references --at` there fails the same way: a member with no symbol is
`UnresolvedTarget` (``"`NAME` has no reference symbol at this site"``,
`selection/references.rs:183-190`); a JSX attribute is `UnsupportedSemantic`
with reason `jsx_typing_required`
(`semantic/evaluation/reference_symbols.rs:1630-1641`); a JSX namespaced
attribute, on any of its characters, is `Internal` today (`canonical
semantic declaration association is not retained`, exit 2); and a semantic
limit or any other semantic failure keeps its own kind.

The `member` outcome carries the member's decoded name (the selected site's
key); the CLI adds the command, with `--project` as the request spelled it
and no `--direct`. Text, on stderr:

```text
FILE:LINE:COLUMN: `NAME` is a member, which --definition does not follow; find its declarations with: tsparse --references --project TSCONFIG --at FILE:LINE:COLUMN
```

The position is the request's, FILE absolute, and the command is rendered by
`command_line::render`. The document (stdout or `-o`):

```json
{"operation":"definition","project":"/abs/tsconfig.json","selection":{"at":{"file":"src/app.ts","position":{"line":4,"column":29}}},"outcome":"member","name":"title","next":{"argv":["tsparse","--references","--project","tsconfig.json","--at","/abs/src/app.ts:4:29"]}}
```

The command selects the same member: it repeats the position through the
same selection owner, which has already selected that member. It does not
promise that the search completes: the search does more work than the
refusal did, and can stop at a semantic limit or report partial coverage
(exit 1) with the declarations it found in its header. A member reached by
`--name` cannot occur: a bare name selects lexically.

### Failures and exit codes

| Outcome | Text | JSON | Exit |
| --- | --- | --- | --- |
| Groups | result, stdout or `-o` | `complete` document, stdout or `-o` | 0 |
| `not_found`, `member` | diagnostic on stderr | document, stdout or `-o` | 1 |
| Refused token: label, `import.meta`, `new.target`, `arguments`, `undefined`, `globalThis`, intrinsic JSX tag, tuple label, import attribute, a type-position qualified member (K4) | ``FILE:L:C: `NAME` is not a supported reference target (REASON)`` | `error` document, kind `UnsupportedTarget` | 1 |
| Name with no declaration among the program inputs, such as an import in a skipped file from a missing module | ``FILE:L:C: `NAME`: REASON`` | `error` document, kind `UnresolvedTarget` | 1 |
| Member position whose member has no symbol | ``FILE:L:C: `NAME` has no reference symbol at this site`` | `error` document, kind `UnresolvedTarget` | 1 |
| Semantic failure while selecting: `UnsupportedSemantic` (a JSX attribute: `FILE:L:C: a JSX attribute relates to its props member only through deferred JSX attribute typing`), `InvalidSemantic`, `SemanticDepthLimit`, `OutputLimit` | `path:line:column: message` | `error` document with `reason`, and `causes` and `omittedCauses` where present | 1 |
| Memory or time limit (`SemanticMemoryLimit`, `SemanticDeadline`; `TSPARSE_MAX_MEMORY_MB`, `TSPARSE_QUERY_TIMEOUT_MS`) | `path:line:column: message`, then `  next: set a higher VARIABLE before retrying` | `error` document with `reason` and `remedy` | 1 |
| Load failure: `Config`, `Syntax`, `Binding`, `UnresolvedImport`, `Unsupported`, `ResourceLimit` | `path:line:column: message` | `error` document | 1 |
| FILE not searched (`OutsideProject`), position outside the file (`InvalidPosition`), dotted name through the library (`InvalidRequest`) | diagnostic naming the operand as spelled | `error` document | 2 |
| Unreadable input (`Io`); internal failure (`Internal`), such as selecting a JSX namespaced attribute today: `canonical semantic declaration association is not retained` | diagnostic | `error` document; an `Internal` one has `reason` `internal_failure`, and `path` and `position` `null` where the failure has no location | 2 |
| Usage error | `CODE: message`, with `  next:` for the dotted name, on stderr | failure document on stdout, never `-o` | 2 |
| Service transport failure | `CODE: message`, then `  why:` and `  next:` where it has them, on stderr | failure document, stdout or `-o` | 2 |
| Result that cannot be written (`write.output`) | `CODE: message` on stderr | the same stderr diagnostic, and no document | 2 |

Every analysis failure is delivered by `delivery::failure` and exits with
`delivery::failure_code` (`src/cli/delivery.rs:434-461`, `:531-618`); a
semantic failure's document adds `reason`, `causes`, `omittedCauses`, and
`remedy` there (`:596-607`, `src/process_limits.rs:366-380`). A diagnostic
`path:line:column: message` prints as much of the location as the failure
carries (`src/analysis.rs:584-604`). A failed write
is reported once on stderr even with `--json`, never followed by a document
(`delivery.rs:393-404`). The `UnsupportedTarget` and `UnresolvedTarget`
messages are the shared selection owners' (`references.rs:232-239`,
`selection/references.rs:183-190`), which name the reference model; one
selection keeps one message. The registry's exit meanings are: 0, "One or
more groups, each with its declarations"; 1, "A missing selection, a member
position (naming the --references search that finds its declarations), an
unsupported or unresolved target, a semantic failure or limit while
selecting, or a project analysis failure"; 2, as `--type`'s.

### Cost

The worker, its checker initialization, and the selection are those of
`--references` at the same selection; the answer then reads `Index::sites`
and builds no occurrence list. So `--definition` costs at most what
`--references` costs for the same selection, and on a warm service both are
dominated by the input check.

## The import hint

When `--show --name NAME FILE` selects nothing, in project mode or file mode,
the not-found answer also names each import or export specifier of FILE that
spells NAME without declaring it, and the command that finds its declaration.
A missing `--at`, a project-wide `--name` without FILE, an ambiguous or found
selection, and a dotted NAME are unchanged.

### Which specifiers match

A specifier matches when its catalog entry has kind `import` or `export`, has
no key, and its name is an identifier whose decoded text is exactly NAME (the
comparison `--name` uses: exact, case-sensitive, after escape decoding, so
`caf\u00e9` matches `café`). By form:

| Specifier in FILE | Entry | Matches NAME |
| --- | --- | --- |
| `import a from "m"` (default import) | `import`, no key, at `a` | yes: `imported` |
| `import { a } from "m"`, `import type { a }`, `import { type a }` | `import`, no key, at `a` | yes: `imported` |
| `export { a } from "m"` | `export`, no key, at `a` | yes: `re-exported` |
| `export { a }` (no `from`) | `export`, no key, at `a` | yes: `re-exported` |
| `import { a as b }`, `export { a as b }` (renaming) | keyed `b` | no: `--name b` finds the entry; `a` is not a local name |
| `import * as ns`, `import x = require(…)` | keyed | no: `--name` finds them |
| `export * as ns from "m"`, `export * from "m"` | no entry | no: nothing in the catalog spells the name (`--definition --name ns FILE` still answers, since the binding index counts `ns` as a candidate, `collect.rs:2629`) |
| `export { "a-b" } from "m"` (string name) | `export`, no key | no: a string name is no identifier token, so `--definition --at` on it selects nothing (K6) |
| `export = x` | `export`, labelled `export=`, no key | no: its label is no identifier |
| `export default x` | no entry | no |

The facts come from FILE's complete catalog: in file mode the catalog `--show`
already built, in project mode one projection of FILE's captured text, the
work `--show --at` does, and only on a miss. The projection, which alone
knows syntax, marks the unkeyed specifier entries whose name is an identifier;
the catalog answers which of them spell NAME; the not-found outcome carries
them, each with its kind and anchor (its name token's start), in source order.
Matching is by syntax alone, in both modes: nothing resolves what a specifier
names, so a file that only says `export { a };` matches `a` though it neither
imports nor declares it. What `a` denotes is `--definition`'s answer.

Projecting FILE again in project mode can fail, as `--show --at` can: the
captured text no longer parses, or the parsing thread cannot start
(`src/project/navigation.rs:271-303`). Such a failure fails the `--show`
request as that input failure, with `delivery::failure_code`'s exit code; it
never becomes a `not_found` answer without specifiers.

### Text

The diagnostic (stderr, exit 1) keeps its first line and adds one line per
matching specifier, in source order:

```text
FILE: no declaration matches `NAME`
`NAME` is imported at FILE:LINE:COLUMN; find its declaration with: tsparse --definition --project TSCONFIG --at FILE:LINE:COLUMN
```

`re-exported` replaces `imported` for an `export` entry. FILE is absolute in
both places. In project mode TSCONFIG is the request's own `--project`
spelling, the command names no `--direct`, and it is complete. In file mode
tsparse knows no configuration, so TSCONFIG is printed literally, the
registry's placeholder, as ambiguity lines print `--at FILE:LINE:COLUMN`. Both
are rendered by `command_line::render`, which quotes any argument that needs
it. A file that both imports and re-exports NAME (`import a from "./m";
export { a };`) gets two lines, the import's first; each command answers for
its own specifier.

### JSON

The `not_found` document gains `specifiers`, present only when one matches,
one object per line above; keys sort as the rest of the show document's do:

```json
{"file":"src/app.ts","operation":"show","outcome":"not_found","project":"/abs/tsconfig.json","query":{"name":"formatTitle"},"specifiers":[{"anchor":{"column":10,"line":1},"kind":"import","next":{"argv":["tsparse","--definition","--project","tsconfig.json","--at","/abs/src/app.ts:1:10"]}}]}
```

`kind` is the entry's kind (`import` or `export`) and `anchor` its position
in the document's `file`. `next` is present only where the command is
complete, so in project mode; a file-mode specifier has `anchor` and `kind`
alone. A document without matches is byte-identical to today's.

### Following the command

The printed position is the specifier's name token, which the binding index
records as a token of the specifier's alias (`collect.rs:2479`, `:2518`,
`:2658`), and step 2 of [selection](#selection) decides there: `--definition
--at` selects the alias's final symbols and prints their declarations, as in
row 2 for an import, row 10 for a re-export, and rows 4 and 5 for a default
import. It prints a failure instead where the binding index has one for that
token: `UnsupportedTarget` where the target is one the lexical model refuses,
such as a JSON property, and `UnresolvedTarget` where it resolves to nothing,
such as an undeclared `a` in `export { a };` or an import from a missing
module in a file TypeScript skips. In a project whose load fails, `--show`
fails the same way first, so no project-mode command is printed. A test
follows every printed command, as for every advertised remedy.

## Ownership

- **Definition selection:** the reference selection owner
  (`selection::references::select`) for the member dispatch, and the lexical
  selection of `project::references` for symbols. The symbol collection of
  `references::lexical` (`references.rs:145-187`) moves into one function
  that both `lexical` and the definition call, so no selection is copied.
  The answer is built from the selected symbols' `Index::sites`; no second
  alias resolver is added.
- **Definition types:** beside the reference types in `crate::references`: an
  answer that is a result (`operation`, `outcome: complete`, `project`,
  `selection`, `groups` of `declarations`) or an unselected outcome
  (`not_found`, or `member` with `name`), reusing `ReferenceQuery` and
  `NamedLocation`.
- **The hint's facts:** the navigation projection and catalog of FILE. The
  not-found outcome (`ShowOutcome::NotFound`) carries the matching
  specifiers, filled by `Project::show_name` when FILE narrows the name and
  by `navigation::file::show_file`. The CLI renders them.
- **Rendering:** a definition renderer beside the references renderer in
  `cli::references`, sharing its location, count, and narrowing helpers; the
  hint in `cli::navigation::show`. Both receive the request's `--project`
  spelling from `Target::Project` (`None` in file mode), and both print
  commands through `command_line::render`.
- **Plumbing:** `ProjectRequest::Definition` and `ProjectResponse::Definition`,
  an `execute_unless` arm, an `operands()` arm (the `--at` file, or FILE with
  `--name`), `Project::definition`, `FlagId::Definition` and
  `OperationId::Definition` with the registry entry above, a request builder
  beside `references()`, `Target::operation` `definition`, and a delivery arm
  in `cli::query`. The service needs nothing per operation.

## Verification

- **Definition:** tests through `Project::execute` and the CLI for each row
  of [cases](#cases) and [out of scope](#out-of-scope-positions), including
  the aliased, local, and namespace-qualified variants of rows 16 and 17,
  expected outcomes from TypeScript 7.0.2 and each K difference pinned to its
  documented answer; each step of [selection](#selection), including a
  parameter property's modifier, a literal member name or key, and each
  character of a JSX namespaced attribute, the colon included, reaching the
  member path, and a non-member literal reaching step 3; the member outcome (text, document, and following its `next`), and a
  member whose selection fails (a JSX attribute's `UnsupportedSemantic`);
  both missing selections; each failure row's code and channel, including a
  `--json -o` result that cannot be written; the usage errors including the
  dotted name's `next` followed; and byte-identical answers from the service
  and `--direct`.
- **Hint:** each matching form and each non-matching form of the table, the
  several-matches case, a file whose only match is an undeclared
  `export { a };`, project and file mode in text and JSON, `next` only in
  project mode and never with `--direct`, an unchanged document without
  matches, a failed projection of FILE failing the request, and every printed
  project-mode command followed to its definition result.
- **Registry:** `tests/cli_registry.rs`, with `COMMANDS.md` and
  `docs/cli-reference.md` regenerated
  (`cargo test --test cli_registry -- --ignored write_reference`).
- The semantic `compare` keeps checking the declaration sets at every
  identifier ([conformance](#conformance)).

The same change updates the agent surfaces `AGENTS.md` lists:
`plugins/tsparse/skills/tsparse/SKILL.md` (where a name is defined, and the
hint), `plugins/tsparse/.claude-plugin/plugin.json` (its description names
the operations it covers, so a new operation changes its scope, and its
version), `SHARED_AGENTS_BLOCK.md` (one line for `--definition`), and
`--show`'s registry details (the hint), never `plugins/codex/tsparse` by hand.

## Examples

A project in `/work`, where every command runs:

```json
{
  "compilerOptions": {
    "strict": true,
    "target": "es2022",
    "module": "esnext",
    "moduleResolution": "bundler"
  },
  "include": ["src"]
}
```

```ts
// src/lib.ts
export function formatTitle(title: string): string {
  return title.trim();
}
```

```ts
// src/index.ts
export { formatTitle } from "./lib";
```

```ts
// src/app.ts
import { formatTitle } from "./index";

const page = { title: formatTitle(" Docs ") };
export const heading = page.title;
```

The comment lines name each file and are not part of it: `formatTitle` is at
`src/lib.ts:1:17`, the import specifier at `src/app.ts:1:10`, the call at
`src/app.ts:3:23`, the re-export at `src/index.ts:1:10`, the property `title`
at `src/app.ts:3:16`, and its use at `src/app.ts:4:29`. TypeScript 7.0.2
answers the first four with `src/lib.ts:1:17-1:28`, and the use of `title`
with `src/app.ts:3:16-3:21`.

**`--definition --at` on the import specifier** (stdout, exit 0):

```text
$ tsparse --definition --project tsconfig.json --at src/app.ts:1:10
/work/tsconfig.json: at /work/src/app.ts:1:10: 1 group, 1 declaration
group 1: formatTitle
  /work/src/lib.ts:1:17
```

With `--json`:

```json
{"operation":"definition","outcome":"complete","project":"/work/tsconfig.json","selection":{"at":{"file":"src/app.ts","position":{"line":1,"column":10}}},"groups":[{"declarations":[{"name":"formatTitle","location":{"file":"src/lib.ts","start":{"line":1,"column":17},"end":{"line":1,"column":28}}}]}]}
```

**On a use** (stdout, exit 0):

```text
$ tsparse --definition --project tsconfig.json --at src/app.ts:3:23
/work/tsconfig.json: at /work/src/app.ts:3:23: 1 group, 1 declaration
group 1: formatTitle
  /work/src/lib.ts:1:17
```

**On the re-export** (stdout, exit 0):

```text
$ tsparse --definition --project tsconfig.json --at src/index.ts:1:10
/work/tsconfig.json: at /work/src/index.ts:1:10: 1 group, 1 declaration
group 1: formatTitle
  /work/src/lib.ts:1:17
```

**`--definition --name` with FILE** (stdout, exit 0):

```text
$ tsparse --definition --project tsconfig.json --name formatTitle src/app.ts
/work/tsconfig.json: `formatTitle` 1 group, 1 declaration; candidates narrowed to /work/src/app.ts
group 1: formatTitle
  /work/src/lib.ts:1:17
```

With `--json`, `selection` is
`{"name":{"name":"formatTitle","file":"src/app.ts"}}` and `groups` is as
above.

**A member position** (stderr, exit 1):

```text
$ tsparse --definition --project tsconfig.json --at src/app.ts:4:29
/work/src/app.ts:4:29: `title` is a member, which --definition does not follow; find its declarations with: tsparse --references --project tsconfig.json --at /work/src/app.ts:4:29
```

With `--json` (stdout, exit 1):

```json
{"operation":"definition","project":"/work/tsconfig.json","selection":{"at":{"file":"src/app.ts","position":{"line":4,"column":29}}},"outcome":"member","name":"title","next":{"argv":["tsparse","--references","--project","tsconfig.json","--at","/work/src/app.ts:4:29"]}}
```

Following it prints today's member search, whose header names the
declaration TypeScript gives:

```text
$ tsparse --references --project tsconfig.json --at /work/src/app.ts:4:29
/work/tsconfig.json: at /work/src/app.ts:4:29: 1 group, 2 occurrences in 1 file; coverage member, complete
group 1: title, declared at /work/src/app.ts:3:16
  /work/src/app.ts:3:16 declaration
  /work/src/app.ts:4:29 use
```

**A position on no identifier**, the `import` keyword (stderr, exit 1;
TypeScript also answers nothing there, and `--show --at` there selects the
statement):

```text
$ tsparse --definition --project tsconfig.json --at src/app.ts:1:1
/work/src/app.ts:1:1: no identifier is at this position; --definition follows identifiers, not keywords or string literals
```

**The `--show --name` miss, project mode** (stderr, exit 1):

```text
$ tsparse --show --project tsconfig.json --name formatTitle src/app.ts
/work/src/app.ts: no declaration matches `formatTitle`
`formatTitle` is imported at /work/src/app.ts:1:10; find its declaration with: tsparse --definition --project tsconfig.json --at /work/src/app.ts:1:10
```

With `--json` (stdout, exit 1):

```json
{"file":"src/app.ts","operation":"show","outcome":"not_found","project":"/work/tsconfig.json","query":{"name":"formatTitle"},"specifiers":[{"anchor":{"column":10,"line":1},"kind":"import","next":{"argv":["tsparse","--definition","--project","tsconfig.json","--at","/work/src/app.ts:1:10"]}}]}
```

On the barrel:

```text
$ tsparse --show --project tsconfig.json --name formatTitle src/index.ts
/work/src/index.ts: no declaration matches `formatTitle`
`formatTitle` is re-exported at /work/src/index.ts:1:10; find its declaration with: tsparse --definition --project tsconfig.json --at /work/src/index.ts:1:10
```

Following either command prints the same group as the first example, with
the header naming that position.

**The `--show --name` miss, file mode** (stderr, exit 1):

```text
$ tsparse --show --name formatTitle src/app.ts
/work/src/app.ts: no declaration matches `formatTitle`
`formatTitle` is imported at /work/src/app.ts:1:10; find its declaration with: tsparse --definition --project TSCONFIG --at /work/src/app.ts:1:10
```

With `--json` (stdout, exit 1):

```json
{"file":"/work/src/app.ts","operation":"show","outcome":"not_found","query":{"name":"formatTitle"},"specifiers":[{"anchor":{"column":10,"line":1},"kind":"import"}]}
```

## Alternatives considered

- **A `--references` flag that omits occurrences.** Cheaper to add, but
  agents ask "where is this defined", not "list references without the
  references", and the flag would have to explain which other references
  flags apply.
- **Excerpts in the definition answer**, as `--show` prints. Saves the agent
  one command, but a synthetic declaration (K2) is no catalog entry, so
  `--show --at` there selects nothing. Locations are correct for every site.
- **Let `--show --name` follow the import.** Changes the meaning of a
  selection whose contract is TypeScript's workspace-symbol keys.
- **Refuse members from the lexical index alone**, without the member
  dispatch. It would skip the per-request checker, a small part of a warm
  query next to the input check (`docs/references.md:101-105`), but the
  index cannot tell where `--references --at` runs a member search: the
  reasons `property` and `enum member` mark both `value.prop` and `E.A` in an
  expression, which are member sites, and `typeof value.prop` and
  `let x: E.A`, where `--references --at` is `UnsupportedTarget`. Only the
  dispatch's syntax check separates them, so a printed remedy would sometimes
  fail.
- **Succeed with no groups on a missing selection**, as `--references` does.
  Reads as "no declaration exists" exactly where the operation was refused.
- **Find the hint's specifiers in the binding index.** Its candidates and
  roles know import and export names, but only in project mode; file mode
  would need a second owner of the same question.

## Not in this design

- Members, call-signature selection, keywords, labels, file references, and
  the other cases of [known differences](#known-differences).
- A hint for a project-wide `--show --name` without FILE: there the question
  is no longer which specifier of a file spells the name, and `--definition
  --project TSCONFIG --name NAME` answers it directly.
- A hint for `export * as ns` (no catalog entry), for a found keyed specifier
  such as `{ a as b }` or a namespace import, and for an edit whose name
  selection misses.
- Definitions in file mode (they need module resolution).
- The `Internal` failure that selecting a JSX namespaced attribute gives
  today. It belongs to the shared reference selection, which `--references
  --at` reaches the same way, and is fixed there, not here.
- A pointer from a `--name` miss to same-named members. `--references --name
  NAME` already counts them and names the `--at` that searches one
  (`unsearched_members`); repeating that advisory in the definition's
  `not_found` would add a field and a line to an outcome the brief does not
  ask to extend, and a bare name's `not_found` already names what it
  searched (declarations, imports, and exports).
- A display limit: every group and declaration is one line, and FILE narrows
  a common name.

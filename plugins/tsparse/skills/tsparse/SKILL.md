---
name: tsparse
description: Navigate, query, and edit TypeScript and TSX code with the `tsparse`
  CLI. Use when working in a TypeScript project (.ts, .tsx, .mts, .cts,
  tsconfig.json, and ES module or single-`module.exports` JavaScript under allowJs) to list declarations in a file (`--outline`) or the
  describe/it/test tree of a test file (`--test-outline`), read a
  declaration's exact source (`--show`), find where an imported or used name
  is declared (`--definition`), find symbol or member references and call
  sites across a project (`--references`), inspect types (`--type`), list the
  object literals that construct a type and the properties each lacks
  (`--constructs`), replace callable bodies or insert statements (`--replace-body`,
  `--insert-after`, `--edit-batch`, `--apply`), or check TypeScript 7 syntax
  (`--check`).
---

# Working on TypeScript Projects with `tsparse`

Use `tsparse` instead of text search or whole-file reads whenever the question or edit targets TypeScript declarations, symbols, members, or types. A text search cannot distinguish a symbol from its spelling, follow import/export aliases, separate shadowed bindings, or resolve receiver types on member accesses; reading an entire file wastes context when you only need its declaration catalog or one function's implementation.

Assume `tsparse` is on `PATH`; it builds and runs on macOS and Linux only. For any flag scope or failure code not covered below, run `tsparse --OP --help` (for example, `tsparse --references --help` or `tsparse --edit-batch EDITS --help`), which prints the operation's exact contract in a few milliseconds without reading files or starting a service.

## Choose the Operation

| Goal | Command | Why `tsparse` instead of text search / raw file reads |
| --- | --- | --- |
| Map what a file declares | `tsparse --outline src/host.ts` | Lists every declaration's `LINE:COLUMN`, kind, and one-line signature indented by containment without reading the full file. |
| Map a test file's suites, tests, and hooks | `tsparse --test-outline src/host.test.ts` | Lists every `describe`/`it`/`test` call and hook at every level as `LINE:COLUMN call name` (`it.each() "adds %i"`); `--outline` lists a test only as its callback (`function it("…") callback`), and by default only two levels deep. |
| Read one declaration's source | `tsparse --show --name RunHost.run src/host.ts` | Prints only that declaration with original line numbers; overloads and shadowed locals stay separate candidates instead of guessing. |
| Find where a name is declared | `tsparse --definition --project tsconfig.json --at src/app.ts:1:10` | Follows import/export aliases and barrel re-exports from an import or a use to the original declarations, printing only their `FILE:LINE:COLUMN`s, at warm-query cost. |
| Find all uses of a symbol | `tsparse --references --project tsconfig.json --name start` | Follows import/export renames and barrel re-exports while keeping shadowed bindings in separate groups. |
| Find uses or callers of a member | `tsparse --references --project tsconfig.json --name RunHost.observe --calls --no-tests` | Matches members by resolved receiver type rather than property spelling, optionally restricted to call sites outside tests. |
| Inspect a declaration or token type | `tsparse --type --project tsconfig.json --at src/host.ts:42:10` | Reports the resolved type summary (and a bounded structural graph with `--expand [--depth 0..32]`). |
| Find every literal that builds a type, e.g. after adding a property | `tsparse --constructs --project tsconfig.json --name RunView --missing lane` | Lists each object literal TypeScript contextually types as the target (annotations, `satisfies`, returns, arguments, array elements, nested values; per union alternative; generic instantiations), with the properties it lacks, optional ones included, which the compiler never reports. By default it examines the literals naming one of the target's keys or none; `--all-literals` examines the rest, slowly. |
| Replace a callable body | `tsparse --replace-body --name RunHost.run src/host.ts --from body.ts --plan run.plan.json` | Replaces only the body, preserves all surrounding bytes, and validates syntax, AST boundaries, `@ts-ignore`/`@ts-expect-error` reach, and JSDoc attachment before writing. |
| Insert statements after a statement or declaration | `tsparse --insert-after --name start src/host.ts --from stmts.ts --plan add.plan.json` | Inserts at the statement seam with matching indentation, line breaks, and automatic semicolon safety. |
| Apply a prepared edit plan | `tsparse --apply run.plan.json` | Verifies every target file's original bytes and inode identity before replacing, rolling back all files if any replacement fails. Replaced files keep only their permission mode. |
| Check if a file or snippet parses | `tsparse --check src/host.ts` | Validates TypeScript 7 syntax in milliseconds and prints a one-line verdict (`ok: ...`). Does not type-check, but rejects a namespace declared with `module` (`module X {}`, TS1540) as TypeScript 7 does. |
| Diagnose a project query or service failure | `tsparse --doctor --project tsconfig.json` | Checks build capabilities, `TSPARSE_*` variables, runtime directory permissions, config readability, the installed TypeScript compiler and the dialect it selects (`compatibility` line), and service state without loading the project. |

**When to use other tools instead:**
- **Text search (`rg` / `grep`):** Searching comments, string literals, markdown/config files, JavaScript that a project refuses (CommonJS beyond one `module.exports = name`, `require`, JSDoc beyond a variable's `@type`; see Troubleshooting), files outside the `tsconfig.json` program, or finding an exact identifier spelling when you only know a substring.
- **Standard file-edit tools:** Editing interface/type/enum member lists, changing function parameters or return type annotations, modifying import specifiers in place, or making sub-statement expression edits.

Definitions and construction queries use the same selected compiler, `baseUrl`/`paths`, JavaScript admission and package redirects as references and types. An import hint names a command at an existing import; it does not propose a new module specifier.

## Select Declarations and Symbols (`--name` and `--at`)

Every selector is exact so a typo or overloaded name never silently targets the wrong code:

- **Exact names (`--name NAME`):** Case-sensitive, decoded identifier match with no fuzzy matching.
  - **Containment (`--name Parent.child`):** In `--show`, `--type`, `--constructs`, `--replace-body`, `--insert-after`, and `--references`, dotted names walk declaration containment (e.g., `createRunHost.startEligible` selects `startEligible` inside `createRunHost`).
  - **Imports and exports:** An import or export specifier has a `--name` key only when it renames (`import { a as b }` has key `b`; `import { a }` has no key). Thus `--show --project tsconfig.json --name createThing` selects the original declaration of `createThing`, not every file that imports it, and `--show --name createThing src/app.ts` misses in a file that only imports it; that miss prints the `--definition` command to run ([below](#find-where-a-name-is-declared---definition)).
- **Lexical vs. semantic member references (`--references`):**
  - A **bare name** (`--references --project tsconfig.json --name run`) performs a fast lexical search across declarations, imports, and export aliases, and ends with a hint listing any matching class/interface/object members it did not search (`N catalog members named run were not searched; search SELECTOR with --at FILE:LINE:COLUMN`).
  - A **dotted name** (`--references --project tsconfig.json --name RunHost.observe`) or **`--at` on a member token** performs a semantic member search across resolved receiver types.
- **Positions (`--at`):** Uses 1-based line and 1-based UTF-16 column.
  - Pass either `--at FILE:LINE:COLUMN` (the exact handle printed by `--show`, `--references`, `--definition`, and candidate lists—paste it directly with no `FILE` operand) or `--at LINE:COLUMN FILE` (the `LINE:COLUMN` printed under a file header in `--outline` or `--test-outline`). Giving a `FILE` operand alongside `--at FILE:LINE:COLUMN` is a usage error.
  - For constructor parameter properties (`constructor(public readonly id: string)`), the identifier `id` selects the lexical parameter; the first modifier (`public`) anchors the semantic class property.
  - **Statements:** Where no declaration is named or anchored, `--at` on a statement's first token (the `it` of `it("adds", () => {...});`, the `expect` of an `expect(...)` line, the `import` of an import) selects that whole statement for `--show` and insertion, reported as kind `statement`. `--outline` does not list statements and they have no `--name`; take the line from `--show`'s line numbers and the column of the statement's first character (its indentation plus one).
  - **Test files:** Start with `tsparse --test-outline FILE` instead of `--outline`. Each listed `LINE:COLUMN` is a test statement's first token, so `tsparse --show --at LINE:COLUMN FILE` prints that whole test and `tsparse --insert-after --at LINE:COLUMN FILE --from new-test.ts --plan PLAN` adds a test after it. It recognizes `describe`/`it`/`test` with Vitest's modifiers and the four `before`/`after` hooks by syntax alone; an `await`-wrapped call, Playwright's `test.describe`/`test.step`, and `suite` are not listed. It reads only FILE and takes no `--project`. JavaScript test files work too; their statements can be shown, but JavaScript editing is refused. File-only navigation ignores project diagnostic policies, including TS1540.
- **Construction sites (`--constructs`):** Select the interface, class, or type alias by `--name` or by `--at` on its declaration name (a type reference in an annotation is `UnsupportedTarget`). Each site line reads `FILE:LINE:COLUMN lacks "lane"?, "status" (unknown)`: keys are quoted, `?` marks an optional property, `(unknown)` a property a spread, index signature, or `any` may provide. Aliases of intersections (`type Props = Base<string> & Extra`) match the literals annotated with them, a type parameter's constraint (and a class's `this`) stands for it, and a literal passed through an inferred type parameter (`const p: Plain = id({...})`) has the inferred type, not the target. Presence is the compiler's type of the literal, not the runtime object. Bases, intersection parts, and `Partial<T>`-style mapped forms are separate types; query them by name. A literal spread into another literal (`{id, ...(c ? {lane} : {})}`) belongs to the outer site. `--missing KEY` keeps the sites lacking the string key `KEY` (exact; `''` is a key); a key the target does not declare as a string key fails with `UnknownProperty` and the nearest key.
  - **Examined literals:** By default only literals with a member naming one of the target's keys, or with no named member (`{}`, only spreads, only computed names), are examined; the header reads `N of M literals examined` and a line counts the rest. Literals whose named members name only other keys can still construct the target (for example a type assertion such as `{other: 1} as Options`, a union context beside the member those keys choose or with a member accepting any key such as `View | {}`, or an excess-property error such as `{...base, other: 1}`): `--all-literals` examines them too, at several times the cost (about 75 s on a ~4,000-file project, past the default 60 s deadline), so narrow it with `--file` or `--no-tests` or raise `TSPARSE_QUERY_TIMEOUT_MS`. A target with an index signature, an empty alternative (`interface Empty {}`), or the global `Object` as an alternative always examines every literal.
  - **Partial answers are normal on real projects:** literals whose context needs flow narrowing, JSX attribute typing, or another unsupported form are located gaps (exit `1`, `coverage partial (N gaps: …)`); every site listed is still exact, and each gap names the literal to inspect by hand.
- **Type positions and `import("…")` types (`--type`):**
  - `--at` on the final name of a type reference (`Box` in `Box<string>`) or of a one-name import type (`Box` in `import("./lib").Box<string>`) reports that whole type with its arguments. The names of a qualified import-type qualifier (`import("./lib").Outer.Inner.Leaf`) are not type anchors (`UnsupportedTarget`); query the declaration or value that uses the type instead.
  - `import("…")` and `typeof import("…")` types resolve as TypeScript 7 does, through aliases, `export =`, type arguments, and a `resolution-mode` attribute. One TypeScript rejects is a located failure (exit `1`), never an `any` answer: a missing module (`module is not resolved`, at the specifier), a missing qualifier name (`namespace has no exported member`, at that name), a module used as a type or value it is not, a circular alias, or a non-string argument fails at TypeScript's diagnostic position, and an alias that resolves to nothing fails at the import type. An import type of a JavaScript `module.exports = name` module follows its `export=` as it does `export =`; a type qualifier `name` does not export is then looked up among the module's own exports, which hold none (the JSDoc `@typedef` exports such a module could add are refused at loading), so it fails as a missing qualifier.
  - Member references find occurrences inside `typeof import("…")` qualifiers (`value.nested` in `typeof import("./lib").value.nested`); a candidate inside a failing import type is reported as a gap. Such a qualifier property name is not a selectable reference seed: select the member by its declaration or a dotted `--name`.
- **Candidate narrowing (`FILE`) vs. occurrence filtering (`--file FILE`):**
  - The positional `FILE` operand narrows which file's candidate declarations `--name` selects.
  - In `--references`, `--file FILE` filters the reported occurrences to `FILE` without narrowing where the symbol is declared. Combine both when needed:
    ```sh
    tsparse --references --project tsconfig.json --name run src/app.ts --file src/api.ts
    ```
- **Ambiguity (`exit 1`):** Overloads, merged interfaces/namespaces, and same-named locals in separate blocks are distinct syntax sites. When `--name` matches more than one in `--show`, `--type`, an edit, or a dotted `--references` name, `tsparse` exits `1` and lists every candidate's `FILE:LINE:COLUMN` and qualified selector. Re-run with the desired candidate's `--at FILE:LINE:COLUMN` or narrower `--name`.
- **Every symbol (`exit 0`):** When any symbol matches, a bare `--references` name and `--definition --name` select every symbol spelled NAME and print one group per symbol: same-named locals are separate groups, and an overload set or merged declaration is one group listing each declaration. Narrow with `FILE` or `--at` when you want one. With no match, a bare `--references` name exits `0` with no groups, while `--definition` exits `1` with `not_found`.

## Find Where a Name Is Declared (`--definition`)

`--definition` answers "where is this declared" from the file that uses the name: it follows import and export aliases (renames, barrel re-exports, `export *`) to the original declarations and prints one group per symbol, one `FILE:LINE:COLUMN` per declaration, and no uses.

- **From an import or a use:** `tsparse --definition --project tsconfig.json --at src/app.ts:1:10`, on an import specifier, a re-export, a call, or any identifier.
- **From a name a file imports:** `tsparse --definition --project tsconfig.json --name useProjectFilesQuery src/features/Dialog.tsx`. FILE narrows the candidates to that file's declarations, imports, and exports; without FILE, every one spelled NAME in the project. NAME is one identifier.
- **Members are refused:** on a property access (`page.title`), a member's name, a literal key, or a parameter property's modifier, it exits `1` and prints a complete `tsparse --references --project … --at FILE:LINE:COLUMN` command, repeating your `--project` spelling, whose group header names the member's declaration (`declared at FILE:LINE:COLUMN`); run it as printed.
- **Nothing selected exits `1`:** a keyword, whitespace, a module specifier, or a name with no candidate is `not_found`, never an empty result.
- **The `--show --name` hint:** when `--show --name NAME FILE` misses a name FILE imports or re-exports without renaming, it prints one line per such specifier: `` `NAME` is imported at FILE:LINE:COLUMN; find its declaration with: tsparse --definition --project TSCONFIG --at FILE:LINE:COLUMN `` (`re-exported` for an export). With `--project` the command is complete, so run it as printed (in `--json`, `specifiers[].next.argv`); without `--project` it names the `TSCONFIG` placeholder, so substitute the project's `tsconfig.json`.

## Work with the Project Service and Cost Model

Project queries (`--references`, `--definition`, `--type`, `--constructs`, and `--outline`/`--show`/edits with `--project`) require an explicit `--project TSCONFIG`. `tsparse` never searches ancestor directories for `tsconfig.json`, and it rejects configs with project `references` rather than searching partially (pass the specific subproject's `tsconfig.json` instead).

`--project` queries automatically start and reuse a per-project background service (keyed by the exact `--project` path spelling and build, expiring after 10 idle minutes). Before each request, the service verifies that no captured source, config, or package manifest changed, rebuilding the snapshot when any input changed.

Typical costs on a ~4,000-file TypeScript project:
- **Single-file `--outline`, `--test-outline`, `--show`, `--check`, or edit without `--project`:** 5–20 ms (no project loaded).
- **Warm project query (`--references` lexical, `--definition`, `--type`, `--outline`, `--show`):** 120–160 ms.
- **Cold start (first project query):** ~6 s (~1.6 GB service RSS).
- **First project query after any file edit:** ~4.5 s (snapshot rebuild).
- **Warm semantic member search (`--references --name Receiver.member`):** ~9–10 s.
- **Warm construction-site search (`--constructs`):** ~14–23 s; with `--all-literals` ~75 s.

These are project-specific measurements, not latency or memory guarantees. A
larger project with 7,896 inputs measured ordinary warm medians of 674–853 ms
and about 3,126 MiB physical footprint after fourteen refreshes. The causes
remain under investigation; account for project size and machine load when
planning queries.

Follow these sequencing rules to keep queries fast:
1. **Omit `--project` when one file suffices:** Use `tsparse --outline FILE` and `tsparse --show ... FILE` without `--project` when you already know the file path.
2. **Batch project reads before editing:** Complete your `--references`, `--definition`, `--type`, and project-wide `--show` queries before modifying files so every query reuses the warm snapshot instead of triggering a rebuild.
3. **Parallelize independent warm queries:** Once the service is running, issue independent read queries concurrently; if the service is cold, let the first query start it so concurrent callers do not wait on the startup lock.
4. **Keep `--direct` for debugging only:** `--direct` (as in `--references --direct`) loads the entire project in the CLI process (~6 s and ~2 GiB RSS every run). Use it only to rule out the service when diagnosing an issue.

## Edit Declarations (`--replace-body`, `--insert-after`, `--edit-batch`, `--apply`)

`tsparse` edits operate in two steps: **prepare** (which validates syntax, AST boundaries, `@ts-ignore`/`@ts-expect-error` governance, and JSDoc attachment, prints a unified diff, and optionally saves a `--plan`) and **apply** (which atomically replaces the target files if their bytes and inode identities are unchanged).

### Supported Sites and Fragment Rules
- **`--replace-body`:** Replaces the complete body of a function, method, constructor, accessor, arrow function, or a variable/property initialized directly with a function or arrow.
  - The fragment (`--from FRAGMENT` or batch `"fragment"`) must be the complete new `{ ... }` block, or an expression for an arrow function.
  - An arrow function returning an object literal must wrap it in parentheses—`({ ok: true })`—because `{ ok: true }` parses as a block with a labeled statement.
  - Interfaces, type aliases, enums, classes, and overload signatures have no callable body (`UnsupportedSite`); to edit a class method, select the method (`--name MyClass.myMethod`), not the class.
- **TypeScript files only:** Every edit operation refuses a JavaScript target (`.js`, `.jsx`, `.mjs`, `.cjs`) with `FILE: JavaScript files cannot be edited; editing needs a .ts, .tsx, .mts, or .cts file` (`Unsupported`, exit `1`), even where the project loads it; `--apply` refuses a plan altered to name one the same way. Edit JavaScript with standard file-edit tools.
- **`--insert-after`:** Inserts one or more complete statements after the statement designated by the selected declaration (a variable binding designates its whole variable statement; an import/export specifier designates its whole import/export declaration). The target statement must live in a file, block, namespace body, or `case`/`default` clause.
  - To insert after another statement in a statement list, such as a sibling `it(...)` test or an `expect(...)` line, select it with `--at` on its first token (`--insert-after --at src/host.test.ts:10:3 --from new-test.ts`); `--test-outline` lists each test's position. A declaration named or anchored there wins: the first token of `x => {};` selects the arrow.
  - A selected callback (an outline entry such as `function it("…") callback`) designates itself and is refused (`UnsupportedSite`); selection never climbs to the call. When its innermost enclosing statement is in a statement list and `--at` on that statement's first token selects only that statement, the refusal ends with `; to insert after the enclosing statement, select it with --at LINE:COLUMN`: re-run with that position. Otherwise it names none, as for a callback in the unbraced body of an `if`, `else`, loop, or label, or `x => {};`.

### Two-Step Workflow

1. **Prepare and save a plan (`--plan PLAN`):**
   - For a single edit, write the replacement body or statements to a temporary file (or pass `--from -` for stdin when `--limit` is not used) and save a plan:
     ```sh
     tsparse --replace-body --project tsconfig.json --name RunHost.start \
       --from /tmp/new-body.ts --plan /tmp/start.plan.json
     ```
   - For multiple edits across one or more files, use `--edit-batch EDITS` so all selections happen against the original files and are validated together:
     ```json
     {
       "edits": [
         {
           "operation": "replace_body",
           "name": "RunHost.start",
           "file": "src/host.ts",
           "fragment": "{\n  return this.launch();\n}"
         },
         {
           "operation": "insert_after",
           "at": "src/host.ts:1:1",
           "fragment": "import { Metrics } from \"./metrics\";"
         }
       ]
     }
     ```
     ```sh
     tsparse --edit-batch /tmp/edits.json --project tsconfig.json --plan /tmp/batch.plan.json
     ```
   - **New-file invariant:** `--plan PLAN` and `-o OUTPUT` on edit commands **must name paths that do not exist yet** (`usage.output` if the file already exists). Delete old plan files or pick a fresh path before re-running.

2. **Apply the plan (`--apply PLAN`):**
   ```sh
   tsparse --apply /tmp/batch.plan.json
   ```
   - Do not modify target files between `--plan` and `--apply`; if any file changed, `--apply` exits `1` (`effect: unchanged`) and you must prepare the plan again.
   - Each file is replaced by a new file that keeps only the original's permission mode: extended attributes, ACLs, owner, and group are not carried over. Files with another hard link or macOS flags (`chflags`) are refused.
   - `tsparse` does **not** type-check edited code. After applying edits, run the project's type checker (e.g., `tsc --noEmit`) or relevant tests to verify types and behavior.

## Read Outcomes, Display Caps, and Failures

### Exit Codes
- **`0` (Success):** Complete query result, valid syntax (`--check`), edit preview/plan saved, or plan applied (`applied` or `no_change`).
- **`1` (No complete answer):** Ambiguous or missing selection, partial semantic coverage (`gaps` or `issues` present), syntax/analysis error, rejected edit (`Boundary`, `Attachment`, `Conflict`, `UnsupportedSite`), stale plan (`unchanged`), or `--doctor` finding.
  - **Always read stdout/stderr on exit `1` before retrying:** an ambiguous selection lists the exact `--at FILE:LINE:COLUMN` candidates to pick from, a `--show --name` miss on an imported name and a `--definition` member position print the next command to run, and a partial reference or type query still delivers the occurrences/graph it resolved alongside the specific gaps.
- **`2` (Usage, I/O, or transport failure):** Begins with a machine-greppable `CODE: message` on stderr (or in `error.code` with `--json`), followed by per-field `issues`, `why:`, and `next:` remedies.

### Display Caps (`--limit` and `-o`)
To protect context windows, stdout responses are capped by default (both in text and `--json`):
- `--references`: `200` occurrences across groups
- `--constructs`: `200` sites, `20` lacking properties per construction, and `50` gaps (one `--limit N` sets all three)
- `--outline`: `200` entries (after `--depth` filtering)
- `--test-outline`: `200` entries
- `--show`: `400` declaration lines
- Edit previews: `400` diff lines per changed file
- Ambiguity diagnostics: `50` candidates

When output is truncated, the last line of text (or the `display` object in `--json`) states how many items were withheld and gives the exact shell-safe command to view all items (`--limit all` or `-o FILE`, where `-o` without `--limit` writes the uncapped output to a file).

### Troubleshooting and Designed Constraints
- **File outside project (`OutsideProject`, exit `2`):** The queried file is in `node_modules`, a declaration library, or excluded by `tsconfig.json`'s `files`/`include`/`exclude`. Use single-file `--outline FILE` or `--show ... FILE` (without `--project`) to inspect files outside the project's searched set.
- **Query memory or timeout limits (`exit 1`):** Read-only queries default to `TSPARSE_MAX_MEMORY_MB=6144` and `TSPARSE_QUERY_TIMEOUT_MS=60000`. A limit failure terminates that service process and its cache. If you change `TSPARSE_*` variables while a service is already running, run `tsparse --references-stop --project tsconfig.json` first so the next request starts a service with the new environment.
- **`module X {}` namespaces (TS1540, exit `1`):** TypeScript 7 parses them but reports TS1540 where it checks the file. Project loading (`--project`) fails with that message at the name in a project file, or a project `.d.ts` without `skipLibCheck`, unless `// @ts-nocheck` or a `@ts-ignore`/`@ts-expect-error` line covers it; dependency declaration files always load and their namespaces resolve. Rewrite a project file's declaration as `namespace X {}`. A project read with the TypeScript 5.6 dialect (below) loads them, as TypeScript 5.6 reports only a suggestion; `--check` reads no configuration and always applies TypeScript 7's rule.
- **TypeScript 5 projects (the TypeScript 5.6 dialect):** The nearest `node_modules/typescript/package.json` above the configuration selects the rules that read it: any 5.x release reads the configuration as TypeScript 5.6.3 does (only 5.6.3 is validated), and no package or any other version as TypeScript 7; TypeScript 7.0.2 still decides every answer, and `tsparse --doctor --project tsconfig.json` prints the selection on its `compatibility` line. Under the 5.6 dialect, an option 5.6.3 does not know (`stableTypeOrdering`, `libReplacement`) or an enabled setting that 5.5 removed (`charset`, `out`, `importsNotUsedAsValues`, …; inactive values such as `importsNotUsedAsValues: "remove"` remain allowed), a `lib` name 5.6.3 lacks (`es2024`, TS6046), and an `include` of `[""]` or `[null]`, which finds no inputs (TS18003), are `Config` errors (exit `1`): remove or correct the setting. A project file's `/// <reference lib>` that names a file rather than a library (`lib.es2015.promise.d.ts`, TS2727) fails loading too (exit `1`): name the library. These are refused as `Unsupported` (exit `1`), naming the option and whether TypeScript 5's default set it: `target` ES5 (the default without `target` unless `module` is `node16`/`nodenext`), `moduleResolution` `classic`/`node10` (the default for `module` `es2015`…`esnext` or `commonjs`), `module` `none`/`amd`/`umd`/`system`, `esModuleInterop`/`allowSyntheticDefaultImports` false, `alwaysStrict` false, `outFile`, `noResolve`, `paths` written as a non-empty array instead of an object, a `paths` pattern `__proto__`, a root-level `__proto__` object, and a `null` `moduleSuffixes` entry; set the option the message names (for `alwaysStrict`, set `strict` or `alwaysStrict`), or remove the entry. A standard library file the installed compiler's `lib` lacks or cannot read fails loading as `Io` (exit `2`) naming the file, as does an unreadable `typescript/package.json`, while one that names no version is `Unsupported` (exit `1`): reinstall or repair that package.
- **JavaScript files:** With `allowJs` (or `checkJs` and no `allowJs`), a project reads `.js`, `.jsx`, `.mjs`, and `.cjs` files (in `node_modules` within `maxNodeModuleJsDepth`, default 0) as plain ES modules or as one top-level `module.exports = name;`, with a variable's JSDoc `@type` as its declared type. Single-file `--check`, `--outline`, and `--show` read them too; only `--check` fails (exit `1`) on TypeScript-only syntax such as a type annotation. In a project, that syntax, and the binding errors `checkJs` and `// @ts-check`/`// @ts-nocheck` report, fail loading (exit `1`). Without `allowJs`, an import of JavaScript is an untyped module, TS7016 under `noImplicitAny` (exit `1`). JavaScript-only meaning is refused where it is written: loading fails with `FILE:LINE:COLUMN: unsupported JavaScript: CONSTRUCT` (`Unsupported`, exit `1`) for other CommonJS (`exports.x`, a second or non-name `module.exports`), `require`, an assignment that adds a member to a function, class, or object (`f.x = 1`), a `this.x =` in a class or object literal member, a JSDoc tag TypeScript turns into syntax other than a variable's `@type` (`@param {T}`, `@typedef`, `@type` on a function, …), a variable's `@type` whose type means something only in JavaScript (`?T`, `*`, `String`, `Object`), or a JSDoc `import("…")` type outside a variable's `@type`, in any project file whatever checks it. A TypeScript 5.x project also refuses `void 0` forms (`exports.a = void 0`) and what TypeScript 5.6 alone reads: `this.x =` in a function or at the top level, `E.prototype.x =`, a top-level `obj.x =`, and JSDoc tags where they stand (`@enum`, `@constructor`, `@readonly` on an object literal member). A project import of a dependency file with one fails at the import (`module "NAME": FILE:LINE:COLUMN: unsupported JavaScript: …`), while a dependency's own import of it is skipped. Exclude the file from the program if the project does not need it, or use text search for it. A default import of a JavaScript script without `import`/`export` syntax or `module.exports`, the types of a CommonJS module's own `module` and `exports`, a JavaScript class extending a generic class, and JSX element types are refused at the query (exit `1`; `javascript_semantics_required`, `jsx_typing_required`).
- **Duplicate package copies (`Unsupported`, exit `1`):** A package installed more than once with the same name, version, and installed peer versions (for example `node_modules/a/node_modules/dup` beside `node_modules/dup`) is one program input, as TypeScript makes it (TypeScript 7 unless `deduplicatePackages` is `false`; TypeScript 5 always): the copy the program meets first, depth first, is its input (both are read), and types and references through either copy reach it. A request that names the other copy's file fails with `FILE: a duplicate copy of package 'dup/index.d.ts@1.0.0', which the program reads from 'node_modules/dup/index.d.ts' instead (TypeScript's package deduplication); name that file`; query that file instead, or read the copy without `--project`.
- **Input that is not a regular file (`Io`, exit `2`):** Project loading (`--project`) refuses a source, `tsconfig.json`, or `package.json` that is a pipe, device, or socket with `PATH: not a regular file` and never reads it; the service reloads and reports the same once an input becomes one. Replace it with a regular file. Single-file parse and navigation without `--project` do not make this check.
- **Service transport errors (`transport.*`, exit `2`):** `tsparse` never falls back silently from a broken service to `--direct`. Run the `tsparse --doctor --project CONFIG` command printed at the end of the error, and follow its `next:` remedy (such as `chmod 700` on the runtime directory or setting a shorter `TSPARSE_SERVICE_DIR` if the Unix socket path exceeds ~100 bytes). If startup cannot record its cause, follow the printed `tsparse --references-serve --project CONFIG` command to run the service in the foreground and see the failure; it runs until stopped or idle.
- **Runtime directory in a sandbox (`transport.directory` or `transport.lock`, exit `2`):** The service's runtime directory is `TSPARSE_SERVICE_DIR` when set; otherwise `$TMPDIR/tsparse-UID` when `TMPDIR` is an absolute path short enough for socket paths; otherwise `/tmp/tsparse-UID`. Where `/tmp` is not writable, set `TMPDIR` (or `TSPARSE_SERVICE_DIR`) to a writable directory, as the error's `next:` line says; `tsparse --doctor` shows which rule chose the directory and why `TMPDIR` was ignored. A lock failure keeps its OS cause and gives the same directory-selection remedy, even when doctor passes its metadata checks; doctor does not test lock writability. Environments with different `TMPDIR` values (an unsandboxed shell and a sandboxed agent) run separate services for one project, and `--references-status`/`--references-stop` see only their own; set the same `TSPARSE_SERVICE_DIR` in both to share one.
- **Surface friction and enhancements:** Do not work around `tsparse` defects or gaps silently. In your final response, report any bugs, misleading messages/remedies, stale results after edits, silent truncation, or missing operations/enhancements that forced you to fall back to text search or raw file reads, along with the command run, its output, and what you expected.

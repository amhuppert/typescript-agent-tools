# Validation history

Earlier validation runs, moved from [validation results](validation.md), which
holds each check's latest result and how to run it. Under each check come
first the results the index has since replaced, newest first, and then the
check's complete record as `validation.md` held it before it became a
current-state index, moved verbatim and in its own order. Only link targets
changed: links to sections of `validation.md` name that file, and one to a
renamed section of [find references](references.md) names its current heading.

## Summary checks

### Rust tests

The full suite passes, with three platform skips for file names that are not UTF-8, which the host's macOS file system refuses (2026-10-20, the agent-facing CLI work's uncommitted tree on `7d3abaf` at its final verification, registered `test` run `vrun-5351079f-dccc-4a09-a088-65b118f14998`).

The full suite passes, with three platform skips for file names that are not UTF-8, which the host's macOS file system refuses (2026-10-20, the agent-facing CLI work's uncommitted tree on `7d3abaf` at step 5b, registered `test` run `vrun-3bf82be9-0a28-42cd-ab5a-a853336512a0`). After that step's review revisions, the changed scope passed with the same skips (`vrun-5ab120c2-b281-481e-84f6-90753a727f9a`: the binary's unit tests and 36 integration tests), and `tests/refusals.rs` passed with no skips (`vrun-08271e04-cc41-4477-93ef-dcecf89f36a1`).

1,547 tests pass (2026-10-17, at `2a59e7c`, the single-thread work's final revision; [its validation](performance-single-thread-opportunities.md#implementation-status)).

421 tests pass on Rust 1.96 (2026-10-08, after the final review's fixes, registered `test` run `vrun-e21adb40`; two platform skips), with one ignored test, `list_configured_program`, which writes a real project's program for `tools/program-inputs.mjs` and `tools/name-selection.mjs` ([Command Center checkpoint](validation.md#command-center-checkpoint)). The probe test now loads 926 maintained compiler probes, 374 of them added for comment directives (see [JSON and configuration probes](validation.md#json-and-configuration-probes)); before the fix 179 of them loaded where `tsc` reports TS2307 or TS2451. The five tests added cover keys of imports and exports (a project-wide name of a function that three files import or re-export without renaming is found; renamed and namespace imports keep their names; a default import and a non-renaming import narrowed to their file are `not_found` while `--at` shows their declarations), one text line per import declaration, `--at FILE:LINE:COLUMN` (relative, absolute, and a Windows-style path read from the right, and the usage errors around it), a dotted reference name as a usage error, and an over-long service socket path. Existing tests now also check the selectors on text ambiguity lines and their pasted locations through show and references in both entries, and the service test of failed refreshes adds a directive cut off by a no-break-space line and by an empty line that U+2028 ends, which answer no operation until repaired. `comment_directives_follow_only_spaces_and_tabs` replaces a test whose premise, ASCII `\s` after `//`, was not TypeScript 7.0.2's. 416 tests passed on 2026-10-08 at plan step 6 (`dfc65e2`, registered `test` run `vrun-b1feb2e5`; two platform skips). Step 6 added a case to `program_order_lists_each_file_after_its_subtasks`: dynamic imports and import types in an `else if` chain, a call's callee and arguments, a variable's type and initializer, a conditional's branches, an operator's operands, and a function type's parameter and return type, in the compiler's source order. 416 tests passed earlier that day, after the round-6 review fixes to merging and reporting (two platform skips), including the 552 maintained compiler probes of `fixtures/json-probes`, 167 of them added: ambient modules declared in several files, JSON values and library members met by module augmentations, when a pattern-matched specifier gets a module of its own, augmentation targets that are not program inputs, and the reporting decision crossed with each diagnostic family (see [JSON and configuration probes](validation.md#json-and-configuration-probes)); the `@ts-check` and `@ts-nocheck` pragma rule; and a missing reference directive in a package or library, now not reported, beside one in a project file. 415 tests passed earlier on 2026-10-08, after the round-5 review fixes to project integration (two platform skips). The seven added cover a 600,000-access member chain whose callback must neither abort nor stall reference loading (a process test that fails on `8edbc1f`: stack overflow in an unoptimized build, over 200 seconds in an optimized one), callback labels of bounded chains, every callee form, and decoded names, in file and project outlines and shows, 5,000 flat callbacks and constants, signatures of 250 nested sites over a 200,000-element array (19 seconds on `8edbc1f`, under one second now, unoptimized), qualified names through dotted and nested namespaces, ambient modules, and global augmentations in a project and narrowed to a file, query paths that are not Unicode beside a real `bad\uFFFD.ts` for every query kind, and a removed working directory answered alike by direct and service entries. 378 passed earlier on 2026-10-08, after the fourth implementation review's fixes to JSON modules, including the 385 maintained compiler probes of `fixtures/json-probes` (see [JSON and configuration probes](validation.md#json-and-configuration-probes)), each loaded as a project whose program and watched names must answer as TypeScript 7.0.2 recorded them, the binder's coverage check on a decorator after a modifier, and an object literal member's single `?` or `!` marker after a byte order mark and each line break. 374 passed on 2026-10-07, after the second rework of module augmentation identity, including the global table built by TypeScript's checker merge with UMD globals and `declare global` blocks, program order against `tsc --listFilesOnly` for import chains, diamonds, reference directives, dynamic imports and import types, type packages, the JSX runtime, and augmentation targets, and the documented differences in `fixtures/reference-exceptions`. The breakdown below is from 217 tests on Rust 1.96 and the minimum supported Rust 1.85 (2026-10-06, after the second design review's fixes; on macOS, Rust 1.85 needs `SDKROOT=$(xcrun --show-sdk-path)` to link): 58 library unit tests (the exhaustive identifier test, pragma, JSON dialect, pattern, path, configuration, source, and resolution tests), 34 parser tests, 13 parser regressions, four literal tests, eight parse-mode CLI tests, 59 reference API tests, 28 reference loading tests, six reference CLI tests, three fixture and marker-grammar tests, two binding-error tests, and two doc tests. Parser coverage includes nested `for` declaration and expression contexts, committed syntax/depth diagnostics, depth exhaustion during speculative parsing, flat comparison lists of 35, 36, and 400 items and a 40,000-item list within a time limit, type-argument closers joined to `=` or `>`, module indicators, `@deprecated` collection after a `#!` line and each line break, file-name rules for JSX and declaration context, malformed hexadecimal escapes, tagged-template cooking and flags, unpaired surrogate preservation, closed stdout for help/version output, deep syntax on ordinary worker-thread stacks and on the deep budget's documented stack, final-tree walking, token spans, UTF-16 offsets, and iterative serialization of a 10,000-operator chain. Reference loading covers uninspectable packages and directory entries, reference directives after every line terminator and with each whitespace character in each position, TypeScript 7's directive attribute rules, a byte order mark before `#!`, JSX pragma whitespace, strict package manifests, `tsconfig.json` comments and escapes, JSON nesting just under, at, and over the limit, package `imports` chains at and past the limit in a dependency, names with a NUL character, and unsupported literal inputs. A test that cannot run on the current platform prints a `SKIP` line that the test gate reports.

### Validation-runner tests

All 353 `tools/*.test.mjs` files pass: 1,117 tests, zero skips, in one registered full `harness-test` run (2026-10-20, the agent-facing CLI work's uncommitted tree on `7d3abaf` at its final verification, `vrun-e8ad414c-f0fc-4e79-80c0-d01dfdec83da`).

All 353 `tools/*.test.mjs` files pass: 1,117 tests, zero skips, in three registered `harness-test` runs, since one run of every file exceeds the server's one-hour limit (2026-10-20, the agent-facing CLI work's uncommitted tree on `7d3abaf` at step 5b: files 1–176 in `vrun-7948ff72-780d-4035-98bb-52cc6d4bac30`, 632 tests; 177–265 in `vrun-b6773e7a-b51a-4ea3-9f07-72d556fb5c18`, 207; 266–353 in `vrun-74dd3053-c3b2-4be8-8d7c-2e9e26cbc42a`, 278). After that step's review revisions, the changed scope passed: 31 of the 353 files, 267 tests (`vrun-ec0e5aa7-8757-4431-abee-58f4aeb319d8`).

All 352 retained harness files pass: 1,084 tests, zero skips (2026-10-16, the T/Q3/Q4 delivery's final validation; [checkpoint](types-completion-correctness-2026-10-13.md)).

127 Node tests pass with the compiler API 6.0.3, the TypeScript 7.0.2 semantic API, and the native compiler configured, with no skips (2026-10-08, after the final review's fixes, registered `harness-test` run `vrun-34a24d12`), including the re-derivation of all 926 maintained probes. The four added, in `tools/navigation-flows.test.mjs`, run the real-project tools on a small project with a built `tsparse` (skipped without one): the flows and name selection pass, a stand-in that alters every `--direct` answer fails the flows at exactly the compared requests, the name census counts a namespace by its last segment and imports and exports as sharing a name, and `tools/program-inputs.mjs` passes a matching listing and fails a reordered one and one with the wrong provenance (with the compilers). 123 passed at plan step 6 (`dfc65e2`, registered `harness-test` run `vrun-b7512f88`): the two added pin `repeated-merges.ts`'s document symbols and check that the `merged` allowance explains neither a module block of another name nor callback copies under variables nothing merged into. 121 passed earlier that day (after the round-5 review fixes; the added test pins `callback-chains.ts`'s document symbols; again after the round-6 review fixes, whose re-derivation covers the 552 maintained probes, now recording diagnostics as `tsc` reports them). 120 passed earlier that day, after the fourth implementation review's fixes to JSON modules, including the re-derivation of every maintained JSON and configuration probe from the compiler, a wrong recorded answer reported and rewritten, and the probe fixtures' shape. 116 passed on 2026-10-07, after the second rework of module augmentation identity, including the real-oracle check of every exception fixture's recorded compiler answers and the native compiler's replacement-library lookup through a symbolic link. 74 passed on 2026-10-06, after the second design review's fixes; without them, the seven real-compiler tests skip with a stated reason. They cover JSON container shape, array length, missing properties, deterministic comparison, deep traversal, process failures, shared syntax eligibility, the shared source comparison and each runner's policy for its facts, native setup/diagnostic failures, unknown runner options and the reported comparison settings, invalid limits, empty selections, invalid fixtures run through failing executables, semantic normalization and its codes, marker parsing and checking, position, role, and name-query comparison, batch-process failures, and the Rust test, Node harness, and semantic validation wrappers, including harness test selection, the summary lines, platform skips, strict mode, and compiler discovery.

### Formatting and lint

`cargo fmt --all -- --check` and Clippy with `--all-targets --no-deps -- -D warnings`, the registered `format` and `lint` commands, pass (2026-10-20, the agent-facing CLI work's uncommitted tree on `7d3abaf` at its final verification; `vrun-cc70017a-e93f-4bda-89c4-1987edbf95d0` and `vrun-d483fc93-d9e3-4210-88ef-594739608f0b`).

`cargo fmt --all -- --check` and Clippy with `--all-targets --no-deps -- -D warnings`, the registered `format` and `lint` commands, pass (2026-10-20, the agent-facing CLI work's uncommitted tree on `7d3abaf` after step 5b's review revisions; `vrun-cb880aeb-3dee-48c5-8944-b5187a3a67c4` and `vrun-ea117ea5-9216-449b-b4f9-b944533c29f9`).

`cargo fmt --all -- --check` and Clippy with `--all-targets --no-deps -- -D warnings`, the registered `format` and `lint` commands, are clean (2026-10-17, at `2a59e7c`; [single-thread work](performance-single-thread-opportunities.md#implementation-status)).

`cargo fmt --all -- --check` and Clippy with `--all-targets --no-deps -- -D warnings` pass (2026-10-08, after the final review's fixes, registered `format` and `lint` runs `vrun-12cf8732` and `vrun-98f567f7`; earlier at plan step 6, `vrun-bb3d8f8d` and `vrun-0d3cad14`).

## Reference semantics

### Marker fixtures

The registered `semantic` command, which runs this check, passed again on 2026-10-20 on the agent-facing CLI work's uncommitted tree on `7d3abaf` at its final verification (`vrun-5aaf5f1e-b46b-4ff1-a978-60b1cb9d5b54`).

The registered `semantic` command, which runs this check, passed again on 2026-10-17 at `2a59e7c` ([single-thread work](performance-single-thread-opportunities.md#implementation-status)).

Measured again 2026-10-08 after the final review's fixes: unchanged, 46 projects, 1,655/1,655 markers. Measured 2026-10-08 at `dfc65e2` (plan step 6, `scripts/validate/semantic-full.sh`), unchanged from the round-6 review fixes: 46 projects, 1,655/1,655 markers. Measured 2026-10-08, after the round-6 review fixes: 46 projects in `fixtures/references`, 1,655/1,655 markers agree with the oracle. The project added, `ambient-module-merges`, covers an `export import` alias in one file's ambient module merging with an interface in a later file's, a shorthand ambient module declared first with a later file's export, and a pattern ambient module declared in two files, one extending the other's interface. Measured 2026-10-08, after the fourth implementation review's fixes to JSON modules, and again unchanged after the round-5 review fixes: 45 projects in `fixtures/references`, 1,628/1,628 markers agree with the oracle. The three projects added cover JSON values through `export =` barrels of every import form, present and missing keys, nested properties, star routes to one property, to one value, and to two files or a declaration, type and `typeof` uses, and empty keys (`json-barrels`); library members of string, number, boolean, and array roots, `null`, `{}`, and empty roots, global and script augmentations of `String`, and star routes to one member or two (`json-library-members`); and members only part of the libraries declare (`json-library-subset`). Measured 2026-10-07, after the global table and program order followed TypeScript's: 42 projects in `fixtures/references`, 1,433/1,433 markers agree with the oracle, every identifier marked and every group marker carrying its roles. Earlier, after the module augmentation merge became one checker merge, 1,399/1,399 markers of 41 projects agreed. The project added with that change, `augmentation-merges`, covers a namespace augmentation merging through an exported import alias, an augmentation through an export of a shorthand ambient module, a module directly inside a script's ambient module augmenting its package, a later augmentation reaching a table an earlier merge moved, a pattern's alias merged one way, and a dependency's conflicting augmentations, where the augmentation replaces an alias and the original declaration stays. Before it, 40 projects with 1,332 markers agreed after standard libraries were loaded. Two projects load standard libraries: `libraries-default` (the default ES2025 libraries; `Array`, `Promise`, `Map`, `document`, and `globalThis.Array`; a module's `declare global` augmentation of `Array` sharing its type parameter with the library declarations; and a script's `interface Window`) and `libraries-explicit` (`lib: ["es5"]` with a `/// <reference lib>` that adds the `Promise` value, and `Map` unresolved); every other project sets `noLib`. Before them, 38 projects with 1,300 markers agreed after the module augmentation identity fix. The two augmentation projects cover augmentations through plain, type-only, and renamed export specifiers, a package's re-export, exported import, and `export *`, two augmenting files sharing a generic type parameter, a class augmentation giving a type-only exported interface a value, an interface beside an exported constant, an `export =` namespace, the module a namespace export names, a pattern ambient module augmented for one specifier, and `global` directly inside a script's ambient module. Projects added with the second design review's fixes cover equally specific pattern ambient modules, a namespace of const enums before its function, an exported interface beside a local namespace, and `\` separators in relative specifiers. They include the marked binding cases formerly inline in `tests/binding.rs`; conflicting, shadowed, diamond, and cyclic star exports; a named re-export cycle; conditional package `exports` with `customConditions` and per-format conditions; `typesVersions` with and without `exports`; a parent-directory `extends` that declares `paths`; `typeRoots` and `types` inside and outside `node_modules`; and `this` in type queries and a member missing from a namespace without exports. `tests/reference_fixtures.rs` checks the same markers, including roles, against the Rust tool.

### Tool comparison

The registered `semantic` command, which runs this check, passed again on 2026-10-20 on the agent-facing CLI work's uncommitted tree on `7d3abaf` at its final verification (`vrun-5aaf5f1e-b46b-4ff1-a978-60b1cb9d5b54`).

The registered `semantic` command, which runs this check, passed again on 2026-10-17 at `2a59e7c` ([single-thread work](performance-single-thread-opportunities.md#implementation-status)).

Measured again 2026-10-08 after the final review's fixes: all 1,655 fixture tokens and 524 name queries match, with the same outcomes. Fixture comparison measured 2026-10-08 at `dfc65e2` (plan step 6), unchanged from the round-6 review fixes: all 1,655 fixture tokens and 524 name queries match, with the same outcomes and normalizations. Measured 2026-10-08, after the round-6 review fixes: all 1,655 fixture tokens and 524 name queries match. Outcomes: 1,208 groups, 346 unsupported, 101 unresolved, with the same normalizations as below. Measured 2026-10-08, after the fourth implementation review's fixes to JSON modules, and again unchanged after the round-5 review fixes: all 1,628 fixture tokens and 515 name queries match. Outcomes: 1,185 groups, 342 unsupported, 101 unresolved. Normalizations: 9 `builtin`, 5 `ambiguousStarExport`, 4 `nonReference:globalAugmentation`, 4 `unresolvedProperty`, 2 `nonReference:thisTypeQuery`, and one each of `missingNamespaceExport`, `nonReference:metaProperty`, and `nonReference:thisParameter`. Measured 2026-10-07, after the global table and program order followed TypeScript's: all 1,433 fixture tokens and 446 name queries match. Outcomes: 1,170 groups, 223 unsupported, 40 unresolved, with the normalizations listed below. Documented differences are separate projects in `fixtures/reference-exceptions`, each recording the compiler's and this tool's answers: `tests/reference_exceptions.rs` checks the tool's, and a real-oracle test in `tools/semantic-oracle.test.mjs` the compiler's, so neither side is a constant. Earlier, after the module augmentation merge became one checker merge: all 1,399 fixture tokens and 436 name queries match, including occurrence roles and every library declaration at its vendored location. Outcomes: 1,163 groups, 200 unsupported, 36 unresolved. Normalizations: 9 `builtin`, 5 `ambiguousStarExport`, 3 `nonReference:globalAugmentation`, 2 `nonReference:thisTypeQuery`, 2 `unresolvedProperty`, and one each of `missingNamespaceExport`, `nonReference:metaProperty`, and `nonReference:thisParameter`. With the library path normalization removed, 13 of the 27 `libraries-default` tokens fail on their library declarations, so the comparison checks them. Group matching ignores a declaration that several groups list, as a one-way pattern augmentation does, as it ignores a token in several groups. Measured 2026-10-05 and not re-run since: the 37 marked temporary projects of `tests/references.rs` and `tests/reference_loading.rs`, kept with `TSPARSE_KEEP_FIXTURES=1`, match on 426 of 436 queried tokens and 116 of 119 name queries. The rest are documented differences: the star re-export diamond with a type-only first route, and a dependency's partly exported overloads (TS2383), which the tool merges leniently into one group with an extra declaration. One further project is a 5,000-level nesting stress file that the tool rejects by design.

### Generated combinations

Measured 2026-10-06, after the second design review's fixes, with the same totals as on 2026-10-05. `tools/reference-combinations.mjs` writes deterministic three-file projects that combine declaration kinds and merges, barrel and import forms, type-only imports, shadowing parameters, locals, and type parameters, and value, type, `typeof`, and qualified uses. Merges are generated only in orders TypeScript accepts. With `--count=400`, all 16,864 tokens and 5,760 name queries match, including roles. Outcomes: 13,857 groups, 2,158 unsupported, 849 unresolved. Normalizations: 567 `unresolvedProperty`, 372 `typeOnlyValue`, 74 `missingNamespaceExport`, 47 `unresolvedReceiver`.

### Adversarial inputs

Part of the Rust tests, last run 2026-10-05, after the second implementation review. Tests with time limits cover branching and shared package `imports` cycles, chains past the 1,024-redirect limit (failing the load as unsupported, also in a dependency), a 10,000-configuration `extends` chain, 8,000 suppressed imports, and 40,000 occurrences on one non-ASCII line. Each loads or queries within its limit.

### JSON and configuration probes

Measured 2026-10-08, after the final review's fixes: every recorded
answer of the 926 agrees with the compiler, and every case passes the Rust
test. Measured 2026-10-08, after the round-6 review fixes: every recorded
answer agrees with the compiler, and every case passes the Rust test.

### Command Center checkpoint

The navigation plan's C checkpoint loads the unchanged Command Center
configuration (`tsconfig.json`: `lib` `dom`, `dom.iterable`, and `esnext`,
bundler resolution, JSON imports, and installed TipTap packages). It was
complete only after standard libraries (step 3a), JSON modules (step 3b), and
augmentation identity (step 3c) all landed, and was repeated after each later
change; the last entry, from plan step 6, is the final proof. Each step's
evidence:

- Step 3c, with commit 09b5d44, used a copy of the configuration with absolute
  `include` paths, `noLib: true`, and a `declare module "*.json"` shim. Two
  position queries on TipTap's `Commands` augmentations in
  `src/lib/prompt-editor` and a name query for `Commands` returned one group
  with 78 declarations: `@tiptap/core`'s interface and its 57 augmentations of
  itself, 16 augmentations in TipTap extension packages, and the project's
  four, the same 78 that TypeScript 7.0.2's semantic API gives the
  declaration. The name query also returns an unrelated local `Commands`. A
  position query inside `node_modules/@tiptap/core` was rejected as outside
  the searched files, as designed.
- After step 3a and the rework of step 3c (2026-10-07, Command Center
  `7706073d2`), the same queries through the configuration with its real
  `lib` and only the JSON shim return the same 78 declarations, in about
  7.7 s and 1.1 GB peak resident memory for a load and one query on the
  machine described under [Performance and memory](validation.md#performance-and-memory).
- Before step 3b, the unchanged configuration failed at its first JSON module
  import, `src/features/session-workflow/fixtures/artemis-bug-reporter-execution.ts`
  2:29 (`Unsupported`).
- After step 3b (2026-10-07, Command Center `7706073d2`, release build), the
  unchanged configuration loads; the C checkpoint is complete. A load with one
  query takes a median 7.70 s (7.66, 7.70, 8.04 s) and about 1.1 GB peak
  resident memory. A name query for `getTicketService` gives one group (one
  declaration, seven occurrences); a position query on `Promise` in
  `src/lib/tickets/service-factory.ts` 319:4 gives the four library
  declarations of `interface Promise` with 6,486 occurrences; position
  queries at the JSON default import `sourceExecution` (2:8) and its use
  (12:38) are `UnsupportedTarget` ("JSON module value"), as TypeScript 7.0.2's
  API gives `export=` with the `Property` flag, which the oracle classifies as
  a member. The `Commands` augmentations in `conversation-mention-node.ts`
  44:13 and `file-mention-node.ts` 12:13 and a name query for `Commands` give
  the same 78 declarations as before (the name query also returns the
  unrelated local `Commands`); a position in `@tiptap/core` is outside the
  searched files, and the augmented member `insertConversationMention` is a
  member, as designed.
- After the second rework of step 3c (2026-10-07, Command Center
  `7706073d2`, release build), which builds the global table and program
  order as TypeScript does, the unchanged configuration still loads, and the
  name query for `Commands` gives the same 78 declarations (58 in
  `@tiptap/core`), byte-identical output to the previous commit's build;
  the `Promise`, `getTicketService`, `React`, `JSX`, and `Window` queries
  give the same groups, apart from the vendored directory's path. Load and
  query took 11.0, 10.4, and 10.2 s for this build and 11.9, 10.9, and
  10.1 s for the previous commit's, run alternately while another process
  loaded the machine, so the change has no measurable cost; peak resident
  memory was about 1.1 GB for both.
- After the fourth implementation review's fixes to step 3b (2026-10-08,
  Command Center `7706073d2`, release build), the unchanged configuration
  still loads, and the `getTicketService`, `Promise`, JSON import
  (`sourceExecution` 2:8 and 12:38, `UnsupportedTarget`, "JSON module value"),
  `Commands` (both augmentations and the name query, 78 declarations), `React`,
  `JSX`, and `Window` queries give byte-identical output to the previous
  commit's build, apart from the vendored directory's path. A load with the
  `getTicketService` query took 7.80, 7.78, and 8.10 s for this build and
  7.80, 7.82, and 7.85 s for the previous commit's, run alternately while
  another process loaded the machine; peak resident memory was about 1.09 GB
  for both.
- After the round-6 review fixes (2026-10-08, Command Center `7706073d2`,
  release build), which merge ambient modules declared in several files,
  JSON values met by augmentations, and pattern-specifier modules as
  TypeScript does, and report a binding diagnostic or missing input only
  where TypeScript reports it in a project file, the unchanged configuration
  still loads, and the `getTicketService` (one declaration, seven
  occurrences), `Promise` (four declarations, 6,486 occurrences), JSON import
  (`sourceExecution` 2:8 and 12:38, `UnsupportedTarget`, "JSON module value"),
  `Commands` (both augmentations and the name query, 78 declarations),
  `React`, `JSX`, and `Window` queries give byte-identical output to the
  previous commit's (`a36413a`, whose code is `84fc747`'s) build, apart
  from the vendored directory's path. A load with the `getTicketService`
  query took 9.03, 9.16, and 9.04 s for this build and 9.21, 9.02, and
  8.99 s for the previous commit's, run alternately; peak resident memory
  was about 1.14 GB for both.
- Plan step 6, the final proof (2026-10-08, Command Center `7706073d2`,
  clean before and after; parser `dfc65e2`, release build). The unchanged
  configuration and the graph it discovers load with no shim, substitute
  root list, or edited source. **Program inputs:** the ignored test
  `list_configured_program` lists the tool's inputs in program order with
  their provenance, and `tools/program-inputs.mjs` compares them with
  `tsc --listFilesOnly` and the semantic API's `getSourceFileMetadata`:

  ```sh
  TSPARSE_PROGRAM_CONFIG=/abs/command-center/tsconfig.json \
  TSPARSE_PROGRAM_LISTING=/abs/listing.json \
    cargo test --offline --release --lib -- --ignored --exact \
      project::program::tests::list_configured_program
  node tools/program-inputs.mjs /abs/listing.json /abs/command-center/tsconfig.json
  ```

  Both list the same 7,639 inputs in the same order with the same
  provenance: 89 standard libraries; 2,225 declaration files of 255
  installed packages, external as TypeScript marks them; and 5,325 searched
  files (4,158 `.ts`, 1,153 `.tsx`, 8 declaration files, including the
  generated `.next/types/routes.d.ts`, `cache-life.d.ts`, and
  `root-params.d.ts` and `next-env.d.ts`, and 6 JSON modules, among them
  `package.json` and a generated model catalog). The first comparison found
  18 files out of place: `scripts/probes/checkpoint-handoff.ts` imports five
  modules dynamically in an `else if` chain, and loading took them in
  reverse; commit `608f2ae` orders a file's requests by position within each
  subtask group, with a regression in
  `program_order_lists_each_file_after_its_subtasks`. No difference remains.
  **References** (each through the service and with `--direct`,
  byte-identical, and identical to the round-6 build's output for the
  queries that run also made): an ordinary
  name, `getTicketService` (one declaration, seven occurrences, the
  compiler's one declaration); the library globals `Promise` in a type
  position at `src/lib/tickets/service-factory.ts` 319:4 (the four
  `interface Promise` declarations, 6,486 occurrences), `Map` in a value
  position at `src/lib/live-references/queries.ts` 39:25 (`declare var Map`,
  977 occurrences), and `document` at
  `src/features/session/tabs/AddConversationMenu.tsx` 116:5 (its one
  `lib.dom.d.ts` declaration, 592 occurrences, as the compiler gives it),
  where TypeScript's one symbol for `Promise` and `Map` holds both the
  interfaces and the variable, which this tool keeps as two groups by
  meaning (see [find references](references.md#loading-and-lexical-differences-from-typescript-702));
  the JSON import `sourceExecution` at 2:8 and its use at 12:38
  (`UnsupportedTarget`, "JSON module value"; the compiler's `export=` with
  the `Property` flag); TipTap's `Commands` at both project augmentations and
  by name (one group of 78 declarations, exactly the 78 the compiler's API
  gives at each position, plus the unrelated local `Commands`); `React`,
  `JSX`, and `Window`; and an unmatched name (no groups, exit 0). The
  semantic `compare` does not run on this project: TypeScript 7.0.2's API
  panics in `getImmediateAliasedSymbol` ("Unexpected nil") while the oracle
  classifies its identifiers, and `compare` classifies every identifier of
  every searched file, with no way to sample files, so the reference checks
  here are these targeted ones. **Navigation** on the same build: see
  [Navigation](validation.md#navigation). **Costs:** [Command Center
  performance](performance-command-center.md#current-state-2026-10-08).
- After the final review's fixes (2026-10-08, Command Center `7706073d2`,
  clean before and after; release build). The whole proof now runs from
  maintained tools that take the project as an argument and need a built
  `tsparse` (the program comparison also needs the compilers):

  ```sh
  TSPARSE_PROGRAM_CONFIG=/abs/command-center/tsconfig.json \
  TSPARSE_PROGRAM_LISTING=/abs/listing.json \
    cargo test --offline --release --lib -- --ignored --exact \
      project::program::tests::list_configured_program
  node tools/program-inputs.mjs /abs/listing.json /abs/command-center/tsconfig.json
  TSPARSE=target/release/tsparse node tools/name-selection.mjs \
    /abs/command-center/tsconfig.json --listing=/abs/listing.json --sample=60
  TSPARSE=target/release/tsparse node tools/navigation-flows.mjs \
    /abs/command-center/tsconfig.json --direct-sample=6 \
    '--request=--references --name getTicketService --json --limit all' \
    '--request=--references --at 319:4 src/lib/tickets/service-factory.ts --json --limit all' \
    '--request=--show --name createMemoryService' … FILE …
  ```

  (`--limit all`, added with display caps after this run, keeps every
  occurrence in the recorded counts, which a reference answer now cuts at
  200 by default.)

  **Program inputs:** the fixed build's listing still equals the compiler's
  7,639 inputs in set, order, and provenance. **Name selection**
  (`tools/name-selection.mjs`): the 5,319 searched TypeScript files have
  14,229 exported top-level declarations whose name no other declaration
  (other than an import or export) has. Before the fix, a project-wide
  `--show --name` found 4,463 of them and was ambiguous for 9,766 (68.6%),
  because imports and re-exports that do not rename shared the name; now it
  finds 14,227. The two left are ambiguous with renaming imports, such as
  `CapabilityCatalogDiagnostic as AgentCapabilityDiagnostic`, which
  TypeScript's workspace symbols also list. 60 sampled names asked through
  each build's service agreed with the count (19 found before, 60 after).
  Over the review's file set, all 5,363 tracked TypeScript files, the same
  count gives the review's 14,530 declarations and 9,917 ambiguous before,
  and 11 ambiguous after. `--show --project tsconfig.json --name
  createMemoryService`, 13 candidates before (12 of them imports in tests),
  finds `src/lib/memory/service.ts:915:17`. **Text outlines** of the 5,363
  files at the default depth: 270,019 entry lines before, 60,176 of them
  imports (22.3%); 241,932 after, 32,089 of them import declarations
  (13.3%); `src/lib/memory/service.ts` went from 342 lines, 63 of them
  imports, to 290 and 11, and JSON is unchanged. **Flows**
  (`tools/navigation-flows.mjs`): step 6's 21 files and
  `src/lib/memory/service.ts`, 22,503 entries: every project outline equals
  the file outline; 377 sampled shows by anchor match the outline and the
  file's bytes; of 181 sampled selectors, 103 are found and 78 ambiguous with
  the entry among the candidates; 432 candidates, JSON ones by file and
  anchor and text ones by their printed `FILE:LINE:COLUMN`, select
  themselves again; 1,481 service requests (mean 415 ms) and 20 of them
  repeated with `--direct`, byte-identical; no failure. **References and
  shows**, each through the service and with `--direct`, byte-identical:
  `getTicketService` (one group, one declaration and seven occurrences),
  `Promise` at 319:4 (four declarations, 6,486 occurrences), `Map` (one,
  977), `document` (one, 592), the JSON import `sourceExecution` at 2:8 and
  12:38 (`UnsupportedTarget`, exit 1), `Commands` at both augmentations (78
  declarations) and by name (78, and the unrelated local `Commands`), an
  unmatched name (no groups), and `createMemoryService` by name and by its
  pasted location. Warm service answers took 388–467 ms and direct answers,
  each a cold load, 8.98–9.23 s. **Document symbols:** all 5,319 searched
  files again have 0 unexplained differences, with step 6's totals.

## Navigation

### Fixtures

The registered `semantic` command, which runs this check, passed again on 2026-10-20 on the agent-facing CLI work's uncommitted tree on `7d3abaf` at its final verification (`vrun-5aaf5f1e-b46b-4ff1-a978-60b1cb9d5b54`).

The registered `semantic` command, which runs this check, passed again on 2026-10-17 at `2a59e7c` ([single-thread work](performance-single-thread-opportunities.md#implementation-status)).

Measured again 2026-10-08 after the final review's fixes (`scripts/validate/semantic-full.sh`): unchanged, with the pinned outlines regenerated identical. Measured 2026-10-08 at `dfc65e2`: the 17 `fixtures/navigation` files have 0 unexplained differences (325 compiler entries, 317 catalog entries, 303 matched; 317 kinds and 303 ranges checked; allowances: 17 merged, 7 transparent, 5 untraversed, 5 shorthand, 4 expando, 4 jsdoc). The added `repeated-merges.ts` repeats one ambient module and `declare global`, which the compiler merges into their first blocks, and gives a property the name of two variables, whose contents the compiler then lists under each. Measured 2026-10-08, after the round-5 review fixes, and again unchanged after the round-6 review fixes: the 16 files then had 0 unexplained differences (303 compiler entries, 295 catalog entries, 285 matched; 295 kinds and 285 ranges checked; allowances: 7 transparent, 5 merged, 5 untraversed, 5 shorthand, 4 expando, 4 jsdoc). The `callback-chains.ts` added then labels callbacks through identifiers, escaped identifiers, private names, `this`, `super`, calls, element accesses, parentheses, a chain of 40 property accesses, and string and template arguments. Measured 2026-10-07: the 15 files then had 0 unexplained differences (283 compiler entries, 275 catalog entries, 265 matched).

### Command Center

Measured again 2026-10-08 after the final review's fixes, release build: the same totals as below, 0 unexplained differences. Measured 2026-10-08 at Command Center `7706073d2` with parser `dfc65e2`, release build: **every** searched TypeScript file, 5,319 files, has 0 unexplained differences (696,497 compiler entries, 668,704 catalog entries, 663,664 matched, every kind and matched range checked; allowances: 28,567 shorthand, 3,767 untraversed, 3,221 merged, 2,647 transparent, 424 expando, 84 truncated), in 64 s. A 69-file sample, adding eight dependency declaration files to the project's own (the ten largest files, the six largest other TSX files, the nine declaration and generated `.next/types` files, `@types/node`'s `fs`, `child_process`, and `http`, `@types/react`, `@tiptap/core`, `next/types/global.d.ts`, `@types/better-sqlite3`, and `zod/v4/classic/schemas.d.cts`, four files of namespaces, augmentations, and `else if` imports, and 32 seeded random files), has 0 unexplained differences (49,915 compiler entries, 48,058 catalog entries, 47,678 matched). Before `dfc65e2` that sample had 116, all from compiler merges the `merged` allowance did not recognize: 56 later `declare module '@tiptap/core'` blocks in `@tiptap/core`'s declarations with their contents, and copies of a property's callback that `scripts/probes/checkpoint-handoff.ts`'s two same-named variables each received. Measured 2026-10-07 at revision `7706073`, with the range and kind checks: a 42-file sample (38,535 compiler entries, 36,709 catalog entries, 36,427 matched) and a 43-file sample including eight declaration files (29,833 compiler entries, 28,601 catalog entries, 28,386 matched) had 0 unexplained differences; see [navigation](navigation.md#differences-from-typescript-702-document-symbols) for their allowances.

### Command Center flows

2026-10-08, after the final review's fixes: `tools/navigation-flows.mjs`, the maintained form of the step-6 scripts, over the same files and `src/lib/memory/service.ts` passes with no failure ([checkpoint](validation.md#command-center-checkpoint)). It adds text round trips through the printed `FILE:LINE:COLUMN` and records any request through both entries. Earlier, at parser `dfc65e2`, through the CLI and the service on unchanged Command Center (session-local `.cc/temp/nav/step6/nav-flows.mjs` and `flows-record.sh`, one-time session evidence now replaced by the tool): 21 files (the three largest tests, two large sources, three large TSX files and a small one, a namespace, a TipTap augmentation, a `declare global`, the seven declaration files and the generated `.next/types/validator.ts`, and `checkpoint-handoff.ts`), 21,772 entries. Each project outline equals the file's file-mode outline. For 323 sampled entries (first, last, deepest, position-only, and a spread), `--show --at` the entry's anchor finds it with the outline's anchor, range, kind, and label; its `source` is byte-identical to the file's text over that range, and the text form prints the same lines with their original line numbers. For the 196 with a selector, a project-wide `--show --name` finds the entry or is ambiguous with it among the candidates (104), and each of 367 candidates, its `file` joined to the directory of `project`, selects itself with `--show --at`. 1,230 service requests; six repeated with `--direct` and 14 recorded flows (outline, a qualified show of an entry 13 levels deep, the 2,312-candidate ambiguity round trip, TSX and declaration files, a missing name, a position on no declaration, a dependency file, and a JSON module) give byte-identical stdout, stderr, and exit status in both entries.

### Project navigation

`tests/project_navigation.rs`, 2026-10-08, after the round-5 review fixes: qualified names through dotted and nested namespaces, ambient modules (including one named `"a.b"`, which no selector spells), and a global augmentation select the same sites across the project and narrowed to a file, with the same selectors in outlines (a mutation that stores dotted keys as one segment now fails it, as it fails file navigation's tests); a query path that is not Unicode is `Unsupported` for outline, narrowed and positional show, and both reference queries, beside a real `bad\uFFFD.ts` that its lossy form names, while exact names of no searched file are `OutsideProject`; a 2,000-segment callback chain keeps its full label in outline and show, and 5,000 flat callbacks and constants outline and select. 2026-10-07: for every declaration of 13 files (12 checked-in fixtures and a declaration file; 14 with `callback-chains.ts` since) loaded as one project, `Project::outline` equals file navigation's entries and each anchor's and selector's `Project::show` equals file navigation's outcome, also across the project; dependency, library, unincluded, missing, and empty paths are `OutsideProject`; cross-file ambiguity orders candidates by file and each candidate's file and anchor select it; positions on no site and invalid positions; 203 and 204 nested functions through the project; a searched JSON module is unsupported navigation input; a session serves source and configuration changes to outline and show as to references, answers neither from an old generation after a failed refresh, and a snapshot answers from its captured text after an edit.

### Command-line compatibility

2026-10-08, after the round-5 review fixes, against `8edbc1f`: 1,153 unoptimized-build invocations (file outline and show in text and JSON, by name and position, missing names and invalid positions, over every navigation fixture; project outline, show, and reference queries by name, narrowed name, and position over every reference fixture's first six files, and outside-project failures) give the same exit status and stderr, and byte-identical stdout except six reference results whose library declarations name the vendored libraries relative to each build's checkout. On unchanged Command Center, optimized builds give byte-identical `--direct` answers to an outline of the largest file (`--json --depth all`, 1,541,232 bytes), a unique-name show (`incumbent.haltReason.message`), and a reference query (`TicketChildSummary`). The intended differences, which these inputs do not reach, are a query path that is not Unicode (now `Unsupported`, never a file) and a working directory that cannot be read (now `Io` in both entries, previously `OutsideProject` directly and a transport failure through the service). 2026-10-07: 116 invocations (reference queries, outline and show in text and JSON over every navigation fixture, failures, and parse mode) gave byte-identical stdout, stderr, and exit status before (`285854a`) and after navigation joined the project snapshot.

## Editing

### Editing Rust tests

2026-10-08: `tests/editing.rs` (sites of both operations, layout and literal interiors, separators, directive and JSDoc relationships, structure, batches and conflicts, positions across every line break, the grammar budget at, under, and over its limit for the original and both candidate parses, and a 40,000-element flat list), `tests/editing_apply.rs` (written bytes; kept permissions, extended attributes (a quarantine value byte for byte), and access control lists, a file's own list in an inheriting directory included, with a directory's inherited list not added, a file's own list denying attribute writes given after them, and an inherited list denying them cleared first; refused macOS flags; stale files of every kind, including a second name retargeted; a change found by the check after staging; and failures injected before staging, inside staging before the backup link, at the first and a later installation, at restoration, and at cleanup, including another writer before restoration), `tests/editing_plans.rs`, `tests/editing_project.rs`, and `tests/editing_names.rs` (staging names other files hold are left alone), and `tests/editing_cli.rs` (request validity, text and JSON forms, plans, channels, exit codes, stdin fragments, results and plans only as new files whatever the outcome, hard links and dangling links included, never creating an absent file the request or plan names (each source of names, the project's configuration included) under any spelling the file system takes for it (links, `.`, case, Unicode forms, `ß`/`ss`), and creating distinct spellings where it does not, the names of a plan that cannot be loaded included and nothing written for one that cannot be read or decoded (a field of the wrong type or shape, a field given twice, text cut short), outputs checked in a directory whose new files cannot be removed until freed of an inherited list, an output left in an append-only directory reported as left with the name it matched, a reserved name that cannot be looked up named, a result that cannot be written after applying, after saving a plan, or after a plan fails to load, and service and direct agreement). The editing, apply, and plan tests also pass on Linux (`rust:1-slim`, Rust 1.99, with `acl` installed) as root and as an ordinary user, where a user extended attribute is kept, a directory's default access control list is not added to a replaced file, a list denying the owner attribute writes is set after them, and a directory's default list without owner write does not refuse the attributes (2026-10-09, as an ordinary user; each test fails without its fix).

### Editing on Command Center

2026-10-09, after the round-2 review fixes, Command Center `7706073d2` (a
shared clone with the checkout's `node_modules` linked and its generated
`build-info.generated.ts` and `.next/types` copied in), release build: four
flows, five files, no failure, with the native 7.0.2 compiler executed on
each changed file before and after its edit and reporting no new diagnostic
code. The first run of the tool, on 2026-10-08, passed its byte and query
checks but invoked the native compiler through Node, which failed and was
ignored, so that run's native claim did not hold; the tool now fails a flow
whose compiler does not run. The timings below are from the 2026-10-08 run on
an otherwise idle machine; the 2026-10-09 rerun, under a load average of
about 7, took 1.2–2.0 s for warm previews, 23–63 s for direct previews, and
37–179 ms to apply. The scenario is maintained as
[`fixtures/editing-flows-command-center.json`](../fixtures/editing-flows-command-center.json):

```sh
TSPARSE=$PWD/target/release/tsparse TYPE_SCRIPT_NATIVE=/path/to/typescript-7/tsc \
  node tools/editing-flows.mjs /path/to/copy/tsconfig.json fixtures/editing-flows-command-center.json
```

| Flow | Edits | Preview (service) | Preview (`--direct`) | Apply | First query after apply | Next query |
| --- | --- | --- | --- | --- | --- | --- |
| A factory's nested function body (`createAgentRunsRepo.createAgentRunRecord`), with a multi-line SQL template kept byte for byte | 1 | 11.7 s (cold service) | 13.5 s | 31 ms | 13.0 s (`--show`) | 0.62 s (`--references`) |
| A TSX component body and a component inserted after another (`src/components/ToastHost.tsx`) | 2 | 0.62 s | 12.9 s | 30 ms | 13.1 s | 0.60 s |
| A large test (`workflow-manager.test.ts`, 10,043 lines): a helper's body and an inserted helper | 2 | 0.69 s | 13.0 s | 106 ms | 13.2 s | 0.61 s |
| Two files: a function inserted after a factory and a factory's body | 2 | 0.61 s | 13.3 s | 53 ms | 13.0 s | 0.60 s |

Preparation through a warm service costs about what a warm show does; a
direct preview loads the project. Applying takes milliseconds. The first query
after applying reloads the project, as after any edit: editing does not
remove the rebuild cost. Diffs are minimal (3 to 10 changed lines per file).

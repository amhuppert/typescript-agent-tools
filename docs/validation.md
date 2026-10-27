# Validation results

Validation uses TypeScript compiler API 6.0.3 for the JSON syntax projection and the native TypeScript 7.0.2 compiler for language acceptance. No differences remain in the checked corpora. The harnesses and authored fixtures are included; compiler installations and external source trees are not distributed with this repository.

Each table below holds a check's latest result, with the date and the revision or run where its record gives them; [validation history](validation-history.md) keeps the earlier runs verbatim.

The T/Q3/Q4 delivery adds a separate fresh-request oracle; it does
not broaden the existing lexical comparison. See
[oracle evidence](types-oracle-evidence.md) for its measured cases and open gates.
Its tests also require Go and a pristine TypeScript Go release checkout at
`typescript/v7.0.2`, commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.
Set `TYPE_SCRIPT_GO_SOURCE` to that checkout, or place it at
`.cache/typescript-go-7.0.2` in the canonical repository. The hook uses a temporary
`go test -overlay`; it never edits the compiler source or adds a production
compiler dependency. Missing or changed source, version mismatch, failed replay,
and missing hook output fail validation.

Frozen isolated resource fixtures replay through `tools/type-oracle-fixture.mjs`.
It rebases only the six captured input-path metadata fields to the explicitly
supplied physical fixture root; semantic identities, source text, ranges,
diagnostics, hashes and external paths remain exact. The original raw goldens
stay unchanged. `tools/type-oracle-fixture.test.mjs` runs real relocated compiler
queries and rejects semantic, source, case, root and external-path drift.

Run the focused oracle suites through the registered command:

```sh
cctl validate run harness-test --queue-if-busy --require-match -- tools/type-oracle.test.mjs
cctl validate run harness-test --queue-if-busy --require-match -- tools/type-oracle-review.test.mjs
```

For request-resource regressions, the registered Rust wrapper accepts the
resource test module or one of its child modules as a single explicit path:

```sh
cctl validate run test --queue-if-busy --require-match -- src/project/semantic/resources_tests.rs
cctl validate run test --queue-if-busy --require-match -- src/project/semantic/resources_tests/literal_cache.rs
```

This runs only that library module with the same resources as every Rust test
run ([selecting Rust tests](#selecting-rust-tests)). It rejects mixed paths and zero executed tests. Use `src/lib.rs` for
the complete library, explicit `tests/<target>.rs` paths for integrations, and
`--scope full` for the complete Rust suite. Narrow resource checks do not replace
the full delivery check.

The fixed Command Center corpus inventory and exact query anchors are in
[manifest-summary.json](../fixtures/types/command-center/manifest-summary.json).
Its inherited/configuration values are distinguished from operation defaults.
[API observations](../fixtures/types/command-center/api-observations.json) are
demand-only compiler observations; their exploratory timings include project
load and do not establish native performance or profile admission.

| Check | Latest result and scope |
| --- | --- |
| Rust tests | The full suite passes, with three platform skips for file names that are not UTF-8, which the host's macOS file system refuses (2026-10-20, the agent-facing CLI work's uncommitted tree on `7d3abaf` after its final verification's second review, registered `test` run `vrun-a38a107f-7f28-4d0f-a342-d4a260578273`). `list_configured_program`, ignored by default, writes a real project's program for `tools/program-inputs.mjs` and `tools/name-selection.mjs` ([Command Center checkpoint](#command-center-checkpoint)), and the probe test loads the maintained compiler probes ([JSON and configuration probes](#json-and-configuration-probes)). Parser coverage includes nested `for` declaration and expression contexts, committed syntax/depth diagnostics, depth exhaustion during speculative parsing, flat comparison lists of 35, 36, and 400 items and a 40,000-item list within a time limit, type-argument closers joined to `=` or `>`, module indicators, `@deprecated` collection after a `#!` line and each line break, file-name rules for JSX and declaration context, malformed hexadecimal escapes, tagged-template cooking and flags, unpaired surrogate preservation, closed stdout for help/version output, deep syntax on ordinary worker-thread stacks and on the deep budget's documented stack, final-tree walking, token spans, UTF-16 offsets, and iterative serialization of a 10,000-operator chain. Reference loading covers uninspectable packages and directory entries, reference directives after every line terminator and with each whitespace character in each position, TypeScript 7's directive attribute rules, a byte order mark before `#!`, JSX pragma whitespace, strict package manifests, `tsconfig.json` comments and escapes, JSON nesting just under, at, and over the limit, package `imports` chains at and past the limit in a dependency, names with a NUL character, and unsupported literal inputs. A test that cannot run on the current platform prints a `SKIP` line that the test gate reports. [Earlier runs](validation-history.md#rust-tests). |
| Validation-runner tests | All 353 `tools/*.test.mjs` files pass: 1,117 tests, zero skips, in three registered `harness-test` runs, since one run of every file can exceed the server's one-hour limit on a loaded machine (2026-10-20, the agent-facing CLI work's uncommitted tree on `7d3abaf` after its final verification's second review: files 1–176 in `vrun-728833a3-69d5-40b1-af0b-65e0c4fe395d`, 632 tests; 177–265 in `vrun-1e4e9313-d5d2-4865-aced-5f7b94aa0e5e`, 207; 266–353 in `vrun-7ff64c45-5d2f-432e-bdf9-652c445cf1a0`, 278). [Earlier runs](validation-history.md#validation-runner-tests). |
| Formatting and lint | `cargo fmt --all -- --check` and Clippy with `--all-targets --no-deps -- -D warnings`, the registered `format` and `lint` commands, pass (2026-10-20, the agent-facing CLI work's uncommitted tree on `7d3abaf` after its final verification's second review; `vrun-628f4038-5722-49d9-9729-a3194cffae0b` and `vrun-aa7b54c3-99d9-43ff-a2e3-934875a9e571`). [Earlier runs](validation-history.md#formatting-and-lint). |
| Authored AST comparisons | 106/106 cases match the compiler API projection, including positions, context flags, and scanner hints (2026-10-24). |
| Authored native checks | 171/171 positive and negative cases match TypeScript 7.0.2, including namespaces declared with `module` (2026-10-24). |
| Upstream native corpus | 14,935/14,935 virtual files pass the acceptance comparison, including 792 invalid cases; 7,574,481 source bytes. Last run in full on 2026-10-17, at `2a59e7c` ([single-thread work](performance-single-thread-opportunities.md#implementation-status)). |
| Upstream API corpus | 14,920/14,920 cases pass, including 14,143 full syntax-tree comparisons and 777 invalid-input rejection checks; 7,568,169 source bytes. Fifteen units using syntax removed in TypeScript 7 are excluded from this TypeScript 6 API oracle and included in the native sweep. Last run in full on 2026-10-17, at `2a59e7c` ([single-thread work](performance-single-thread-opportunities.md#implementation-status)). Since 2026-10-24, namespaces declared with `module` count as TypeScript 7 syntax rather than removed syntax: their trees are compared and `--check` must reject them (TS1540); the full corpus has not been re-run since. The TypeScript-Go checkout's own `testdata` cases (389 units at `2bd066d`) pass, `moduleKeywordSkipLibCheck.ts` included, except `jsxMultilineAttributeStringValues2.tsx`, whose multi-line attribute strings the TypeScript 6 API rejects and the parser accepted before and after this change. |
| Generated grammar combinations | 852/852 cases pass, including 101 invalid combinations, with positions, flags, and scanner hints (2026-10-24). |
| Standard declarations | All 108 `.d.ts` files shipped with TypeScript 7.0.2 parse successfully; 3,785,246 bytes. |
| Compiler/declaration AST sweep | 185/185 files match positions, flags, and scanner hints; 13,185,321 bytes. This includes 77 compiler source files and 108 standard declarations. |
| Identifier classification | The exhaustive native baseline covers all 1,114,112 Unicode code points for identifier-start and identifier-part acceptance. The current Rust test checks every code point against the recorded fixture. The updated native harness also passes a 4,096-code-point smoke check in an isolated output directory. |
| Robustness | 3,980 deterministic truncation, mutation, and adversarial cases return successfully or produce a syntax diagnostic, with no crashes or timeouts. Includes deeply nested class heritage, assignment probes, function types, conditional types, mapped types, templates, and other recursive grammar paths at and beyond the nesting budget. |
| Compiler schema | The generated Rust kinds/constants and documented kind table pass the pinned compiler API's regeneration check. |

The upstream corpus is `tests/cases/conformance` and `tests/cases/compiler` at TypeScript 7.0.2 commit `1e4744d68260a7cb91b62b12edc3f6a2187faaf1`. The compiler-source corpus is from TypeScript 6.0.3 commit `050880ce59e30b356b686bd3144efe24f875ebc8`.

The upstream native harness uses `--noCheck --noResolve --noEmit` to isolate syntax. Namespaces declared with `module` (TS1540) and missing operands after `throw` also receive strict checks because the compiler defers these errors to its checker. Authored tests confirm the corresponding native diagnostics. API runners share one classifier for TypeScript 7's removed syntax, deferred errors, and ignored AMD module-name comments.

The four API runners (`conformance`, `grammar-corpus`, `source-corpus`, and `upstream-corpus`) also share one comparison, `compareSource` in [the oracle module](../tools/reference-oracle.mjs). It parses a source with the compiler API and, when TypeScript 7 still has its syntax, with the Rust CLI. A source TypeScript 7 parses whole has its tree compared, including a namespace declared with `module`, whose only error is the checker's TS1540. Invalid inputs, that namespace among them, and every input under upstream `--check-only`, run with `--check`. It returns facts: eligibility, validity and diagnostics, whether the tree is compared (`parsed`), the CLI outcomes, and the first disagreement with the oracle. A disagreement is a process failure, accepted invalid syntax, rejected valid syntax, or the first differing tree path from `firstDifference`. Each runner decides what those facts mean:

- Authored AST fixtures fail unless they are eligible and valid.
- Generated grammar combinations fail when ineligible.
- Source and upstream runners count removed syntax separately.
- The grammar, source, and upstream runners pass an eligible invalid input when the parser rejects it.

The native runners compare native diagnostics rather than syntax trees, so they do not use this comparison. Comparisons omit the compiler's binding data, reference/pragma metadata, JSDoc trees, and lazy caches; see [the JSON contract](ast-json.md) and [normalization code](../tools/ast-comparison.mjs). The API runners reject any option they do not read, so a misspelled `--positions`, `--flags`, or `--scanner-hints` fails instead of comparing less, and each summary line and JSON report names the compared settings, for example `compared: positions, flags, scanner-hints`, `structure only`, or, under upstream `--check-only`, `acceptance only`.

## Focused Rust targets

### Relative path preparation

`fixtures/paths/relative.json` records 172 calls to TypeScript 7.0.2's
`tspath.GetRelativePathToDirectoryOrUrl`. The Rust library checks both individual
results and their unions over importer directories, including root kinds,
normalization, separators, cwd, and sensitive/insensitive Unicode comparisons.
The native check uses the same pristine Go source verification as the type
oracles above and the test-only overlay in `fixtures/paths/oracle_test.go`:

```sh
cctl validate run harness-test --queue-if-busy --require-match -- tools/relative-path-oracle.test.mjs
cctl validate run test --queue-if-busy --require-match -- src/lib.rs
cctl validate run test --queue-if-busy --require-match -- tests/project_path_allocations.rs
```

To add cases, add input rows to the fixture and run the native check. It writes
the compiler's complete answers to `.cc/temp/relative-path-native.json` before
comparing them with the fixture; inspect those answers before updating the
expected fields. The allocation regression loads sibling index modules through
`Project::load`, checks the final references, and bounds allocation counts and
requested allocation bytes. It covers ordinary relative spellings; `rootDirs`
still uses its existing per-directory transforms.

### Session reuse

The library tests in `project::session::reuse_tests` and
`project::program::resolve::reuse_tests` check reuse through published session
responses and compare them with fresh projects. They cover source/fact/navigation
sharing, current file identities, module status and library roles, JSON and BOM
changes, synthetic requests, obsolete-input release, resolution invalidation,
and failures, including a type reference's origin as part of its reuse
identity, a `baseUrl` target appearing and disappearing, a type root gaining a
name, and an installed compiler version change that selects other package
files. Deterministic resolver counters distinguish reuse from recomputation.
The existing session integration tests also exercise exact observations, FIFO
reads, and files changing during construction.

```sh
cctl validate run test --queue-if-busy --require-match -- src/lib.rs tests/reference_session.rs tests/project_service.rs
```

For the resolution tests alone, select
`src/project/program/resolve/reuse_tests.rs`. This exact existing file is accepted
only on its own and must execute at least one test; mixing it with another target
fails before Cargo runs. It uses the same resources as every Rust test run. Until
the wrapper change reaches the canonical checkout, exercise that new selector
locally with `scripts/validate/test-changed.sh src/project/program/resolve/reuse_tests.rs`;
Command Center resolves registered wrappers from the canonical checkout.

### Selecting Rust tests

The registered `test` command accepts explicit Cargo test roots. Use
`cctl validate run test --queue-if-busy --require-match -- src/lib.rs` for the
library tests, `src/main.rs` for the binary's unit tests, or `tests/<target>.rs`
for integration tests; they can be combined. Apart from the explicitly supported
resource and reuse test modules, other source paths are rejected before Cargo
starts. An explicit run records nothing.

With no explicit paths, the changed command runs the targets that the changes
since the last passing run can affect (`select_rust_tests` in
`scripts/validate/common.sh`):

| Changed path | Runs |
| --- | --- |
| `tests/<target>.rs` | that test; nothing once it is deleted |
| `src/main.rs`, `src/cli/`, `src/service/`, `src/process_limits.rs` | the binary's unit tests and every test that runs the binary (`CARGO_BIN_EXE_tsparse` or `tests/support/service.rs`) |
| `docs/`, `tools/`, `scripts/`, `memory-bank/`, `README.md`, `AGENTS.md`, `CommandCenter.json`, `LICENSE`, `.gitignore` | every test, and the binary's unit tests, whose source names that directory or file |
| anything else: library sources, `tests/support/`, fixtures, `vendor/`, Cargo and build settings | every target, with the doc tests |

The base is the last tree a run without explicit paths passed for, recorded with
the Rust toolchain (`rustc -Vv`) in `target/validation/rust-tests-passed.tree`,
or else the merge base with the target branch; a different toolchain runs every
target. A run records its tree only when it passed and no file changed while it
ran. A binary module that the library also declares, a `#[path]` that
reaches one, or a support module other than `service.rs` that runs the binary
makes a binary change run every target. The full command runs every target and
records its pass; merge validation still runs the full suite.

Both commands build the selected targets once with four jobs
(`cargo test --no-run`), then run four test executables at a time with two test
threads each, longest first by their last durations
(`target/validation/rust-test-durations`). An integration test whose file has a
line beginning `// validation: run alone`, such as one that asserts wall-clock
limits, runs after the others, one at a time. Every executable runs even after
one fails, and a failure replays the complete output of each failed executable.
On the tree of 7d3abaf (285 test executables), the full suite took 316 s with one
thread per executable, against 734 s for `cargo test` running one executable at a
time with two threads; a change to `src/cli/` alone took 35 s. On 2026-10-19,
with 292 executables, two threads per executable took the full suite from 384 s
to 273 s. Tests build with `opt-level = 1` (`[profile.test]` in `Cargo.toml`):
that day the executables' time fell from 1,069 s to between 274 and 347 s, a
rebuild after a library change cost about the same as before, and a build from
scratch took 475 s instead of 183 s with 12 jobs.

## Harness tests

The registered `harness-test` command runs the Node tests in `tools/*.test.mjs` on two workers. `scripts/validate/harness-test-full.sh` runs all of them and records its pass. `scripts/validate/harness-test-changed.sh` runs only the paths it is given; each must name an existing `tools/<name>.test.mjs`, and any other path exits 2 before a test runs, because a runner such as `tools/robustness-corpus.mjs` would execute as a corpus tool and another module would pass trivially. An explicit run records nothing.

Without paths, the changed command runs the test files that the changes since the last passing run can affect, as [`tools/harness-selection.mjs`](../tools/harness-selection.mjs) decides from each test file's closure: the file and every module it reaches through a relative import or a module path it names.

| Changed path | Runs |
| --- | --- |
| `tools/<name>.test.mjs` | that file |
| another module under `tools/` | the test files whose closure holds it, and those that name `scripts/` when a validation script names it |
| `src/`, `tests/`, `examples/`, `benches/`, `vendor/`, `build.rs`, `Cargo.toml`, `Cargo.lock`, `.cargo/` | the test files whose closure runs the Rust tool or Cargo (it names `tsparse`, `cargo`, `reference_batch`, or a `target/` directory: 27 of 352 on 2026-10-19), those that name the changed file, and for `vendor/` those that name `vendor/` |
| `fixtures/` | the test files whose closure names the fixture; a directory below `fixtures/<area>/` that holds it, such as a fixture project whose other files a test names; `fixtures/`, `fixtures/<area>`, or another holding directory as a whole (followed by a quote or `${`); or a `'fixtures'` path segment |
| `scripts/`, `docs/`, `memory-bank/`, `plugins/` (the agent plugins and their generated packages), top-level Markdown such as `README.md`, `AGENTS.md`, and `SHARED_AGENTS_BLOCK.md`, `CommandCenter.json`, `LICENSE`, `.gitignore`, and other files under `tools/` | the test files whose closure names that directory or file |
| anything else, such as package files | every test file |

The base works as for Rust tests, recorded in `target/validation/harness-tests-passed.tree` with the Node version and each compiler variable's value, size, and time; a change to any of them runs every test file. `test` and `harness-test` allow 60 minutes (`timeoutMs` in `CommandCenter.json`); other commands get Command Center's global default. Most harness tests check the pinned compilers, so their time is the compilers': in a 58-file sample, two workers took 607 s, four 564 s, and eight 384 s at a load near 13.

Compiler variables that are set are used as they are. Unset ones are filled from the canonical checkout's `.cache/compiler-oracles`: `TYPE_SCRIPT_API` from `typescript-6.0.3/package/lib/typescript.js`, and the TypeScript 7.0.2 semantic API and native compiler as for the [`semantic` command](#reference-semantics). `TYPE_SCRIPT_56_API`, behind the [5.6.3 tools](#typescript-563-oracle), is filled from `typescript-5.6.3/package` the same way, and strict mode's check before the run and the compiler values recorded with a pass include it. A test whose compiler is still missing skips with a stated reason. On success the wrapper prints one line, for example:

```text
harness tests: 74 total, 67 executed, 67 passed, 7 skipped (HARNESS_REQUIRE_COMPILERS=1 fails on skips)
```

A failure replays the complete test output, and a run in which no test executed fails. The Rust `test` command reports the same way: on success one line, `rust tests: passed, 0 skipped`, or the count followed by each `SKIP name: reason` line from a test that cannot run on this platform, such as a permission test run as the superuser or a case-policy test on a case-sensitive file system; a failure replays Cargo's build diagnostics or each failed executable's output. A run without paths first prints which targets or files it selected and why. `HARNESS_REQUIRE_COMPILERS=1` selects strict mode. In strict mode, a missing compiler exits 2 before any test runs and names what was not found, and any skipped test fails the run with a list of the skipped tests and their reasons. Values other than `1`, `0`, or unset exit 2.

## Reference semantics

Find references is checked against the TypeScript 7.0.2 language service through its unstable sync API, separately from the syntax oracles above. `tools/semantic-oracle.mjs` classifies every identifier in a project's searched files and normalizes TypeScript's symbols to the tool's documented model (see [find references](references.md#loading-and-lexical-differences-from-typescript-702)). `tools/semantic-conformance.mjs markers` checks that marker comments in fixture projects agree with the oracle, including marked roles.

`compare` drives the Rust query API through `examples/reference_batch.rs`. It issues a position query at every oracle token, which must return the token's groups, with the same occurrence names and syntactic roles, or the expected `UnsupportedTarget` or `UnresolvedTarget`. It then issues one name query for each distinct spelling among the name candidates (declaration names, local import names, and exported alias names). A name query must return the union of its candidates' groups or, when a candidate is unsupported or unresolved, fail with that candidate's error kind at its location. Differences are reported by kind, including repeated occurrences or declarations within a group.

```sh
export TYPE_SCRIPT_SEMANTIC_API=/path/to/typescript-7.0.2/package
export TYPE_SCRIPT_NATIVE=/path/to/typescript-<platform>-7.0.2/package/lib/tsc
cargo build --offline --example reference_batch
node tools/semantic-conformance.mjs markers
node tools/semantic-conformance.mjs compare [PATH ...]
```

Both the API package and the native binary must report exactly 7.0.2. Reports are written to `.cache/semantic-markers.json` and `.cache/semantic-conformance.json`; differences and setup failures exit 1. The real-oracle Node tests skip, with a stated reason, unless both variables are set.

The `compare` summary, printed last and saved in the report, has these fields:

- `tokens`, `queried`, `passed`, `failed`, and `byKind`: position queries and their differences by kind.
- `names`: `queried`, `passed`, `failed`, and `byKind` for name queries.
- `outcomes`: queried tokens by expected outcome (`groups`, `unsupported`, `unresolved`, or `anomaly`).
- `normalizations`: queried tokens whose expected outcome departs from TypeScript's own resolution under a documented difference. The codes are `nonReference:<syntax>` (TypeScript resolves a `this` parameter, tuple label, binding key, meta-property, `declare global`, or similar non-reference name), `typeOnlyValue`, `ambiguousStarExport` (two `export *` declarations supply the name with different targets; TypeScript reports TS2308 and keeps the first), `builtin` (`undefined`, `arguments`, and `globalThis`), `missingNamespaceExport`, `unresolvedProperty`, `unresolvedReceiver`, `receiverType`, and `declarationOutsideTarget` (a parameter property's name).

A change in these counts shows that tokens moved between outcomes or into a normalization even when every comparison passes.

Standard library targets are ordinary symbols: their groups include their library declarations, and library files, like external-library files, are not searched. The native compiler reads its libraries from its own directory, and the tool from the byte-identical copies in `vendor/typescript-7.0.2-lib`. The oracle locates a default-library file from the compiler's directory at the vendored copy of the same name; this is the only path normalization, so any other difference in a library declaration still fails. A library replaced by an `@typescript/lib-*` package keeps its own path. Fixtures that model a program without libraries say `noLib` explicitly.

The vendored data has three checks. `node tools/vendor-typescript-libs.mjs --check` compares every vendored file with the native package and its archive, and with the typescript-go checkout at tag `typescript/v7.0.2` (`TYPE_SCRIPT_GO_SOURCE`, or `.cache/typescript-go-7.0.2` in the canonical checkout), whose `enummaps.go` supplies the `LibMap` and target defaults recorded in `provenance.json`; without `--check` it regenerates the directory. The Rust unit tests in `src/references/project/libraries.rs` check the tool's catalog against `provenance.json` and the vendored file set. `tools/vendor-typescript-libs.test.mjs` checks the recorded hashes and, with `TYPE_SCRIPT_NATIVE`, that the vendored files are the native compiler's, that its TS6046 message lists the recorded library names in order, that each name selects its recorded file, and that each target loads its recorded default.

The registered `semantic` validation command, `scripts/validate/semantic-full.sh`, builds `reference_batch` and `tsparse`, runs `markers` and `compare` over `fixtures/references`, and then runs the [navigation](#navigation) comparison over `fixtures/navigation`. It takes the compilers from the three variables (`TYPE_SCRIPT_SEMANTIC_API`, `TYPE_SCRIPT_NATIVE`, and `TYPE_SCRIPT_API`) or, when they are unset, from the canonical checkout's `.cache/compiler-oracles`: `typescript-7.0.2/package`, the native package for the current platform, `typescript-<platform>-<arch>-7.0.2/package/lib/tsc`, or the only native 7.0.2 package present, and `typescript-6.0.3/package/lib/typescript.js`. When one is missing it says so and exits 2: the comparison did not run, which is never a pass. It runs in `preMerge` after the Node harness tests. Command Center resolves registered wrappers from the canonical project root, so the registration and wrapper take effect once they are integrated there; before that, run the wrapper directly.

| Check | Result and scope |
| --- | --- |
| Marker fixtures | 46 projects in `fixtures/references`, 1,655/1,655 markers agree with the oracle, last counted 2026-10-08 after the final review's fixes. A 47th, `module-keyword-namespaces` (namespaces declared with `module` in dependencies under `skipLibCheck`, keyword names after a dot included), was added on 2026-10-24: its 34 markers agree with the oracle, and the registered `semantic` command passed with it. The registered `semantic` command, which runs this check, passed again on 2026-10-20 on the agent-facing CLI work's uncommitted tree on `7d3abaf` after its final verification's second review (`vrun-867ee4be-2750-450c-96af-fd0f92fa5abd`). They include the marked binding cases formerly inline in `tests/binding.rs`; conflicting, shadowed, diamond, and cyclic star exports; a named re-export cycle; conditional package `exports` with `customConditions` and per-format conditions; `typesVersions` with and without `exports`; a parent-directory `extends` that declares `paths`; `typeRoots` and `types` inside and outside `node_modules`; and `this` in type queries and a member missing from a namespace without exports. `tests/reference_fixtures.rs` checks the same markers, including roles, against the Rust tool. [Earlier runs and the projects each added](validation-history.md#marker-fixtures). |
| Tool comparison | All 1,655 fixture tokens and 524 name queries match, last counted 2026-10-08 after the final review's fixes. Outcomes: 1,208 groups, 346 unsupported, 101 unresolved. Normalizations: 9 `builtin`, 5 `ambiguousStarExport`, 4 `nonReference:globalAugmentation`, 4 `unresolvedProperty`, 2 `nonReference:thisTypeQuery`, and one each of `missingNamespaceExport`, `nonReference:metaProperty`, and `nonReference:thisParameter`. The registered `semantic` command, which runs this check, passed again on 2026-10-20 on the agent-facing CLI work's uncommitted tree on `7d3abaf` after its final verification's second review (`vrun-867ee4be-2750-450c-96af-fd0f92fa5abd`). Documented differences are separate projects in `fixtures/reference-exceptions`, each recording the compiler's and this tool's answers: `tests/reference_exceptions.rs` checks the tool's, and a real-oracle test in `tools/semantic-oracle.test.mjs` the compiler's, so neither side is a constant. Group matching ignores a declaration that several groups list, as a one-way pattern augmentation does, as it ignores a token in several groups. Measured 2026-10-05 and not re-run since: the 37 marked temporary projects of `tests/references.rs` and `tests/reference_loading.rs`, kept with `TSPARSE_KEEP_FIXTURES=1`, match on 426 of 436 queried tokens and 116 of 119 name queries. The rest are documented differences: the star re-export diamond with a type-only first route, and a dependency's partly exported overloads (TS2383), which the tool merges leniently into one group with an extra declaration. One further project is a 5,000-level nesting stress file that the tool rejects by design. [Earlier runs](validation-history.md#tool-comparison). |
| Generated combinations | Measured 2026-10-06, after the second design review's fixes. `tools/reference-combinations.mjs` writes deterministic three-file projects that combine declaration kinds and merges, barrel and import forms, type-only imports, shadowing parameters, locals, and type parameters, and value, type, `typeof`, and qualified uses. Merges are generated only in orders TypeScript accepts. With `--count=400`, all 16,864 tokens and 5,760 name queries match, including roles. Outcomes: 13,857 groups, 2,158 unsupported, 849 unresolved. Normalizations: 567 `unresolvedProperty`, 372 `typeOnlyValue`, 74 `missingNamespaceExport`, 47 `unresolvedReceiver`. [Earlier runs](validation-history.md#generated-combinations). |
| TypeScript compiler | Recorded with commit a44bc42 (2026-10-05), from position queries before roles and name queries were compared; not re-run since. A copy of the TypeScript 6.0.3 compiler sources (43 files, 9.4 MB of modules), with `noLib`, shorthand ambient declarations for its Node.js imports, and a stub for its build-generated diagnostics file: all 365,899 tokens match. |
| Local projects | Recorded with commit a44bc42 (2026-10-05), from position queries before roles and name queries were compared; these private projects are not available to re-run. Nine configurations from seven TypeScript and TSX applications and libraries, with installed packages, each loaded through a configuration that extends the original with `noLib` and keeps its `node_modules`: all 62,170 tokens match (24,283; 14,259; 9,675; 4,881; 4,035; 3,663; 1,273; 90; 7). Three other configurations are rejected as documented: a JSON module import, TypeScript 7's removed `baseUrl`, and a package missing from `node_modules`, which TypeScript also reports. |
| Adversarial inputs | Part of the Rust tests, which passed in full on 2026-10-20 (Rust tests, above). Tests with time limits cover branching and shared package `imports` cycles, chains past the 1,024-redirect limit (failing the load as unsupported, also in a dependency), a 10,000-configuration `extends` chain, 8,000 suppressed imports, and 40,000 occurrences on one non-ASCII line. Each loads or queries within its limit. [Earlier runs](validation-history.md#adversarial-inputs). |
| Corpus robustness | Recorded with commit a44bc42 (2026-10-05); not re-run since. Each of the 12,444 upstream conformance and compiler cases loads as a one-file project without a panic. Rejections are invalid TypeScript 7 (`with`, misplaced exports), duplicate declarations, or imports of the case's other virtual files. |

### JSON and configuration probes

`fixtures/json-probes/*.json` holds 926 small projects whose expected outcomes
come from the pinned compilers: TypeScript 7.0.2's program files in
`GetSourceFiles` order, its diagnostics with positions as `tsc` reports them
(a file's binding diagnostics among its semantic ones, so none for a file
TypeScript skips), the symbol each watched identifier denotes, and, for
configuration cases, `tsc --showConfig` values. They cover program admission
of resolved files (`admission.json`, 20 cases), configuration conversion with
`null`s, empty strings, lists, inheritance, and empty keys
(`config-merge.json`, 46), configuration text and repeated options
(`configuration.json`, 35), JSON inputs, options, and import forms
(`inputs.json`, 72), what JSON names denote through barrels, star routes, type
positions, and library members (`linking.json`, 50), and the JSON grammar and
its diagnostic positions (`parsing.json`, 162). Five files from the round-6
review cover merging and reporting: ambient modules declared in several files,
by alias form, file arrangement, order, shorthand declaration, star export,
and pattern (`ambient-merges.json`, 23); augmentations meeting a JSON value,
property, or library member directly or through a star barrel, with const,
value namespace, and interface declarations and dependency arrangements
(`json-augmentations.json`, 18); when an augmentation of a pattern-matched
specifier makes a module of its own, before and after merges through
namespace, type, and value aliases, with a pattern in one or two files and a
shorthand pattern (`pattern-augmentations.json`, 17); augmentation targets
that only another route adds (`augmentation-targets.json`, 9); and the
reporting decision, `noCheck`, `skipLibCheck`, `@ts-nocheck` in its pragma
variants, comment directives, and dependency files, crossed with duplicate
declarations, script globals, augmentation conflicts, TS2671, TS2428, TS2440,
missing modules, reference directives, and augmentation targets, TS2481,
TS2395 and TS2383, TS2434, TS2669, and UMD globals, with controls where one
side of a two-sided diagnostic stays reported (`reporting.json`, 100). Two
files from the final review cover the comment-directive rules: which lines a
directive reaches across (`suppression-lines.json`, 280), `// @ts-ignore` and
`// @ts-expect-error` before one intervening line, blank or a `//` comment,
after an empty, space, tab, no-break space, vertical tab, form feed, or byte
order mark prefix, with every line ended by LF, CR, CRLF, U+2028, or U+2029,
before a missing module and before each of two conflicting declarations; and
what a directive is (`suppression-directives.json`, 94), each of those space
classes after `//` and at the start of a block comment's last line after each
line break, with controls such as `////@ts-ignore`, `@ts-ignored`,
`@TS-IGNORE`, a directive on a block comment's first line or on the error's
own line, and directive text in a template, string, or regular expression.
Their `tool` fields expect a failure exactly where `tsc` reports TS2307 or
TS2451. `tools/json-probes.mjs`
re-derives every recorded answer from the compilers (the same
`TYPE_SCRIPT_SEMANTIC_API` and `TYPE_SCRIPT_NATIVE` as the semantic oracle) and
exits 1 on any difference; `--update` records new or changed cases and
`--only=NAME,...` selects them:

```sh
node tools/json-probes.mjs [--update] [--only=NAME,...] [fixtures/json-probes/FILE.json ...]
```

The Rust test `compiler_probe_projects_load_and_answer_as_recorded` loads each
project with `Project::load` and requires the recorded program and an
answer at each watched name that follows the documented model: no symbol is
`UnresolvedTarget`; a JSON value or property, or an interface member, is
`UnsupportedTarget`; any other symbol is a group with its declarations. A
case's `tool` field states where this tool's outcome is a failure, `error` with
the kind and the compiler diagnostic (or position) it reports, or where a
documented difference changes an answer, `names` with a `note` naming it. 460
cases carry such a field, 52 of them with a note. A case's answers hold on a
case-insensitive file system; `upper-file-files`, whose `Data.JSON` is a JSON
input only where TypeScript folds case, adds `caseSensitive`, the `native` and
`tool` that replace them where file names keep their case, which the Rust test
reads after `Case::detect` and the runner after the compiler's `toPath`. `tools/json-probes.test.mjs`
runs the re-derivation in the harness tests, which skip it without the
compilers. Measured 2026-10-08, after the final review's fixes: every recorded
answer of the 926 agrees with the compiler, and every case passes the Rust
test ([earlier runs](validation-history.md#json-and-configuration-probes)).

### Command Center checkpoint

The navigation plan's C checkpoint loads the unchanged Command Center
configuration (`tsconfig.json`: `lib` `dom`, `dom.iterable`, and `esnext`,
bundler resolution, JSON imports, and installed TipTap packages). It was
complete only after standard libraries (step 3a), JSON modules (step 3b), and
augmentation identity (step 3c) all landed, and was repeated after each later
change. The latest run follows; [validation history](validation-history.md#command-center-checkpoint)
keeps each step's evidence, including plan step 6's final proof with the
provenance of the program inputs and why the semantic `compare` does not run
on this project.

After the final review's fixes (2026-10-08, Command Center `7706073d2`,
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

File navigation is checked against TypeScript 7.0.2's document symbols, which only its language server provides. `tools/navigation-oracle.mjs` speaks the language-server protocol to `tsc --lsp --stdio` (the native compiler, which must report `Version 7.0.2`), and `tools/navigation-conformance.mjs` compares `tsparse --outline --json --depth all` with its symbols, classifying entries by the TypeScript 6.0.3 syntax tree (`TYPE_SCRIPT_API`):

```sh
node tools/navigation-conformance.mjs [--report=PATH] fixtures/navigation [PATH ...]
```

Entries are matched by selection position and label. Every catalog entry's kind must name the syntax at its range in the fine vocabulary (`type` a type alias, `import` an import declaration, `module` an ambient module or global augmentation, `accessor` an accessor). Matched entries must agree on the compiler's `SymbolKind`, on range (equal to the compiler's, except that a variable binding's statement and a specifier's declaration must contain it), and on containment. The other differences each need a documented allowance decided by the entry's own syntax (`shorthand`, `transparent`, `merged`, `untraversed`, `expando`, `jsdoc`, and `truncated`; see [navigation](navigation.md#differences-from-typescript-702-document-symbols)); no allowance passes to descendants, so the declarations inside an omitted expando assignment must still match, contained where the assignment is. The report, `.cache/navigation-conformance.json` by default, counts each allowance and the kinds and ranges checked; any unexplained difference exits 1.

The `semantic` command runs this comparison over `fixtures/navigation` with the `tsparse` it builds. The harness tests cover the comparison itself: `tools/navigation-oracle.test.mjs` pins each fixture's document symbols (`<file>.symbols.json`) and exercises the server lifecycle against stand-in servers (a silent or failing `initialize`, an abnormal exit after shutdown, and a request failure whose cleanup also fails); `tools/navigation-conformance.test.mjs` mutates the catalog entries pinned in `<file>.outline.json`, which `tests/navigation_cli.rs` keeps equal to the tool's output, and requires the comparison to fail for a dropped expando function or its contents, a reparented declaration, a listed shorthand, a wrong range, and a wrong fine kind.

| Check | Result and scope |
| --- | --- |
| Fixtures | The 17 `fixtures/navigation` files have 0 unexplained differences (325 compiler entries, 317 catalog entries, 303 matched; 317 kinds and 303 ranges checked; allowances: 17 merged, 7 transparent, 5 untraversed, 5 shorthand, 4 expando, 4 jsdoc), measured at `dfc65e2` and again, unchanged, after the final review's fixes (2026-10-08, `scripts/validate/semantic-full.sh`), with the pinned outlines regenerated identical. The registered `semantic` command, which runs this check, passed again on 2026-10-20 on the agent-facing CLI work's uncommitted tree on `7d3abaf` after its final verification's second review (`vrun-867ee4be-2750-450c-96af-fd0f92fa5abd`). [Earlier runs and the files each added](validation-history.md#fixtures). |
| Command Center | **Every** searched TypeScript file, 5,319 files, has 0 unexplained differences (696,497 compiler entries, 668,704 catalog entries, 663,664 matched, every kind and matched range checked; allowances: 28,567 shorthand, 3,767 untraversed, 3,221 merged, 2,647 transparent, 424 expando, 84 truncated), in 64 s, measured at Command Center `7706073d2` with parser `dfc65e2` and again, with the same totals, after the final review's fixes (2026-10-08, release build). At `dfc65e2`, a 69-file sample, adding eight dependency declaration files to the project's own (the ten largest files, the six largest other TSX files, the nine declaration and generated `.next/types` files, `@types/node`'s `fs`, `child_process`, and `http`, `@types/react`, `@tiptap/core`, `next/types/global.d.ts`, `@types/better-sqlite3`, and `zod/v4/classic/schemas.d.cts`, four files of namespaces, augmentations, and `else if` imports, and 32 seeded random files), has 0 unexplained differences (49,915 compiler entries, 48,058 catalog entries, 47,678 matched). [Earlier runs](validation-history.md#command-center). |
| Command Center flows | 2026-10-08, after the final review's fixes: `tools/navigation-flows.mjs`, the maintained form of the step-6 scripts, over step 6's 21 files and `src/lib/memory/service.ts` passes with no failure ([checkpoint](#command-center-checkpoint)). It adds text round trips through the printed `FILE:LINE:COLUMN` and records any request through both entries. [Earlier runs](validation-history.md#command-center-flows). |
| Project navigation | `tests/project_navigation.rs` passes in the latest full Rust run (Rust tests, above). For every declaration of 14 files (13 checked-in fixtures and a declaration file) loaded as one project, it checks that `Project::outline` equals file navigation's entries and that each anchor's and selector's `Project::show` equals file navigation's outcome, also across the project. It also covers qualified names through namespaces, ambient modules, and a global augmentation, across the project and narrowed to a file; `OutsideProject` paths and query paths that are not Unicode; cross-file ambiguity; positions on no site and invalid positions; long callback chains and flat lists; and a session serving source and configuration changes to outline and show. [Earlier runs and what each added](validation-history.md#project-navigation). |
| Command-line compatibility | 2026-10-08, after the round-5 review fixes, against `8edbc1f`: 1,153 unoptimized-build invocations (file outline and show in text and JSON, by name and position, missing names and invalid positions, over every navigation fixture; project outline, show, and reference queries by name, narrowed name, and position over every reference fixture's first six files, and outside-project failures) give the same exit status and stderr, and byte-identical stdout except six reference results whose library declarations name the vendored libraries relative to each build's checkout. On unchanged Command Center, optimized builds give byte-identical `--direct` answers to an outline of the largest file (`--json --depth all`, 1,541,232 bytes), a unique-name show (`incumbent.haltReason.message`), and a reference query (`TicketChildSummary`). The intended differences, which these inputs do not reach, are a query path that is not Unicode (now `Unsupported`, never a file) and a working directory that cannot be read (now `Io` in both entries, previously `OutsideProject` directly and a transport failure through the service). [Earlier runs](validation-history.md#command-line-compatibility). |

## TypeScript 5.6.3 oracle

The TypeScript 5.6 dialect's acceptance is checked against the pinned TypeScript 5.6.3 compiler API. It is an oracle for that dialect alone and never replaces the 6.0.3 syntax oracle or the 7.0.2 semantic oracle. `TYPE_SCRIPT_56_API` names the 5.6.3 package directory, the one holding `package.json` and `lib/`; unset, the tools look in the canonical checkout's `.cache/compiler-oracles/typescript-5.6.3/package`, found from any linked worktree through `git rev-parse --git-common-dir`. The package's `package.json` must say 5.6.3; a missing or other package exits 2 with the reason. [`tools/ts56-compiler.mjs`](../tools/ts56-compiler.mjs) owns the lookup and the 5.6.3 configuration parse for every tool.

With `incremental: true`, 5.6.3 writes `.tsbuildinfo` into the project whenever it checks or emits, even with `--noEmit`. Every 5.6.3 `tsc` run therefore passes `--incremental false`, and every program built through the API has `incremental` forced false. A `composite` configuration cannot run the program comparison, since 5.6.3 refuses `--incremental false` there (TS6379). Even so, never run either tool on a real project checkout: clone it under `$TMPDIR` with `cp -cR` and run on the clone.

`tools/program-inputs.mjs --oracle=5.6` compares the Rust listing of `list_configured_program` with 5.6.3's program, as the default mode does with 7.0.2's. The order comes from `lib/tsc.js -p TSCONFIG --listFilesOnly --incremental false`, which the API's program must list too. Provenance comes from `isSourceFileDefaultLibrary` (`library`) and `isSourceFileFromExternalLibrary` (`dependency`). A library from the package's `lib` directory is named by its file name, and any other file by its canonical path relative to the configuration's directory. 5.6.3 lists a package-id duplicate (a second installed copy of the same package name and version, a SourceFile with `redirectInfo`) as an input of its own, where 7.0.2 lists only the first copy. The 5.6 mode therefore treats the copy as a spelling of `redirectInfo.redirectTarget`: it leaves the order and the file set, the target keeps its own position (5.6.3 lists a copy it meets while its target's own imports are processed before the target, where 7.0.2 lists only the target, after those imports), and the report (default `.cache/program-inputs-5.6.json`) lists each copy and its target under `redirects`. The listing `list_configured_program` writes records this tool's redirects too, which must name the same copies with the same targets (a `redirect` difference otherwise). Without `--oracle`, the 7.0.2 comparison is unchanged.

`tools/ts56-oracle.mjs` answers position queries through the 5.6.3 language service, over a host built from the parsed configuration. `type FILE:LINE:COLUMN` reports `getQuickInfoAtPosition` as its `displayPartsToString` text and span. `references FILE:LINE:COLUMN` reports each `findReferences` reference's file, one-based start, and TypeScript's `isDefinition` and `isWriteAccess`. `specifier IMPORTER=TARGET` reports `moduleSpecifiers.getModuleSpecifiers` for TARGET's module from IMPORTER with the preferences the checker's `getSpecifierForModuleSymbol` gives it, the module names the TypeScript 5.6 dialect's spelling tests expect; an `index` collision question resolves its name against the tool's working directory, as 5.6.3's host does. FILE, IMPORTER, and TARGET are relative to the configuration's directory. LINE and COLUMN are one-based, with the column in UTF-16 code units as for tsparse's `--at`, and lines break where 5.6.3's line map breaks them. It prints one JSON document. A bad query exits 1 before any answer, and a run that cannot start exits 2. Harness tests call `ts56Oracle(config, queries)` for the same document.

```sh
cp -cR /abs/app "$TMPDIR/app"
TSPARSE_PROGRAM_CONFIG=$TMPDIR/app/tsconfig.json TSPARSE_PROGRAM_LISTING=$TMPDIR/listing.json \
  cargo test --offline --release --lib -- --ignored --exact project::program::tests::list_configured_program
node tools/program-inputs.mjs "$TMPDIR/listing.json" "$TMPDIR/app/tsconfig.json" --oracle=5.6
node tools/ts56-oracle.mjs "$TMPDIR/app/tsconfig.json" type src/a.ts:1:14 references src/a.ts:1:14
```

The TypeScript 5.6 dialect's option catalog (`src/project/program/config/catalog_5_6.rs`) is data `tools/ts56-option-catalog.mjs` extracts from the same package's `lib/typescript.js`: `optionDeclarations` with each list's element declaration, enumeration keys and the keys TS6046 lists, and the `--build`-only names, with the file's SHA-256. `node tools/ts56-option-catalog.mjs` rewrites it and `--check` exits 1 when it differs; `tools/ts56-option-catalog.test.mjs` re-extracts it byte for byte, checks the recorded hash, and checks every enumeration's TS6046 list and every list element's name against the diagnostics the compiler reports, skipping with the reason when the package is not found.

The dialect's library catalog (`src/project/program/libraries/catalog_5_6.rs`) is data `tools/ts56-library-catalog.mjs` extracts the same way: `libs` in declaration order, its two repeated names at both places, `libMap` with the last file of a repeated name, and `getDefaultLibFileName` for each `target` key and for any other target, with the file's SHA-256. The declarations themselves are not extracted: the dialect reads the installed compiler's `lib` directory. `node tools/ts56-library-catalog.mjs` rewrites it and `--check` exits 1 when it differs; `tools/ts56-library-catalog.test.mjs` re-extracts it byte for byte, checks the recorded hash, the 88 names, 86 files, and target defaults, that every cataloged file is in the package's `lib` and every `/// <reference lib>` in one names a library, skipping with the reason when the package is not found. The Rust unit tests in `src/project/program/libraries.rs` check the catalog's provenance and its program-order positions, and `src/project/program/config/catalog.rs` that the option catalog's `lib` enumeration is the same `libMap`.

Library selection under the dialect is pinned in Rust at `Project::load` and query results, against 5.6.3's `tsc --listFilesOnly --incremental false` and `tools/ts56-oracle.mjs` on scratch copies, beside 7.0.2's answers for the same projects: the program tests in `src/project/program/tests.rs` (the libraries Artemis's `lib` and `target` select, which `tools/program-inputs.mjs --oracle=5.6` also compares, each target's default, `/// <reference lib>` names and their TS2726 and TS2727 messages, a missing or unreadable installed file, library and dependency roles, replacement packages, and program order around them, where routes overlap, and by where a library lies) and `tests/typescript_5_6_libraries.rs` (binding, `--type` answers through the service and directly, overload answers that program order decides, including where two routes reach one file, where a replacement or a referenced file lies beneath the installed `lib`, however its reference path is spelled, and where the package or its `lib` is a link, a replacement's `typesVersions` read with each dialect's version, session refreshes, including a package whose `package.json` links elsewhere and is relinked and a linked `lib` retargeted, and the CLI from another directory). They install a copy of the pinned 5.6.3 package as the project's compiler, found as the tools find it, and skip with the reason when it is not found. Where an answer depends on where the installed libraries lie, reproduce it with the project's own copy: its `lib/tsc.js`, or `tools/ts56-oracle.mjs` with `TYPE_SCRIPT_56_API` naming the project's `node_modules/typescript`, since the oracle otherwise reads the libraries of the package it loads.

Module and type-reference resolution under the dialect is pinned in Rust the same way, against 5.6.3's `tsc --listFilesOnly --traceResolution --incremental false` and native 7.0.2's on scratch copies, run from the copy's real path (native `tsc` keeps a symbolic-link working directory as given): `tests/typescript_5_6_resolution.rs` (`paths` before `baseUrl`, a matched pattern's missing substitutions, exact and wildcard keys and ties, `rootDirs`, `typesVersions` patterns, the declaring file of `baseUrl` and `paths`, ranges selected by the installed version and by 7.0.2, and an Artemis-shaped project through `Project` queries and the CLI, whose answers are `tools/ts56-oracle.mjs`'s), `tests/type_reference_roots.rs` (type roots and the origin of a type name, under both dialects), and the session refresh tests named above. Module names the checker spells are pinned in `src/project/program/tests.rs` against `tools/ts56-oracle.mjs specifier` and against TS2305 messages, in which 5.6.3's `tsc` and native 7.0.2 spell a module afresh when a CommonJS file's only imports of it use the `import` mode. `--type` output never shows these names: both compilers' quick info names a module by its file whenever no module resolver host is given, as `--type` does. A `baseUrl` name the checker needs is therefore proven in two parts: the generated names equal 5.6.3's `getLocalModuleSpecifier` (those spelling tests), and `a_type_query_spells_a_base_url_module_from_captured_inputs` in `tests/typescript_5_6_resolution.rs` shows a `--type` request that demands such a name completing from captured inputs alone. Those `tsc` runs replace the fixtures' `noLib` with a `lib` (`"lib": ["es2022"]`): without libraries `tsc` reports TS2318 for the missing global types and then no semantic diagnostic at all.

Configuration reading is pinned in Rust instead. Each configuration experiment, run on a scratch copy through 5.6.3 (`getParsedCommandLineOfConfigFile`, and `tsc --showConfig` or `--noEmit`, with `--incremental false`) and native 7.0.2 (`tsc --showConfig` and `--listFilesOnly`), is a loading assertion under both dialects in `src/project/program/config/conversion_tests.rs` that names its experiment: the effective options, the searched files, or the failure. `fixtures/configuration/base-url-project` is a synthetic `tsconfig.json` with the option and pattern shapes of Artemis's, under neutral names: `baseUrl` `.`, six base-relative `paths` patterns (an `@/*` alias, three `@x-y/*` aliases, a `prefix*` pattern, and an exact pattern into a package's `dist`), the same compiler options and value shapes, `include` and `exclude` patterns of the same kinds, `watchOptions`, a one-entry `files` list, and a `ts-node` section with its own `compilerOptions`. It sits beside a 5.6.3 manifest, stubs of the three library files its `lib` names, and two source stubs; the tests compare its effective options with 5.6.3's `--showConfig` of a copy, and `tests/typescript_5_6_configuration.rs` checks that the TypeScript 7 dialect still refuses its `baseUrl`, as `Unsupported`, where native 7.0.2 reports TS5102. No tracked fixture holds Artemis's files. Expected outcomes come from those runs, never from this tool's output; where the tool deliberately differs, the test says so and [references](references.md#compiler-selection) records it.

`tools/ts56-oracle.test.mjs` and `tools/program-inputs-56.test.mjs` run both tools on small projects under the system's temporary directory. They cover quick info and references through an import, each line break and an astral character's UTF-16 columns, module specifiers through `baseUrl`, bad queries and their exits, a library's name, the duplicate package's redirect, a redirect 5.6.3 lists before its target, and the absence of `.tsbuildinfo` although the projects set `incremental: true`. With the 7.0.2 compilers, one test also checks that the folded 5.6.3 program is the one 7.0.2 lists. The tests skip, with the reason, when the 5.6.3 package is not found. Run each file through the registered `harness-test` command.

JavaScript inputs are pinned in Rust at `Project::load` and query results, against native 7.0.2 and 5.6.3 on scratch copies (`tsc --pretty false --listFiles --incremental false`, `tools/type-oracle.mjs`, `tools/ts56-oracle.mjs`, and declaration emit, `--declaration --emitDeclarationOnly`, which shows whether a JavaScript assignment or JSDoc tag gave an export a member or a type): `tests/javascript_admission.rs` (inputs by `allowJs` and `maxNodeModuleJsDepth`, including a file reached deeply and then shallowly, untyped modules, constants modules' types and references), `tests/javascript_syntax.rs` (TypeScript-only syntax in each JavaScript extension, under every checking option, file-local commands), `tests/javascript_checking.rs` (each `checkJs` state against each pragma, both dialects), `tests/javascript_semantics.rs` (plain ES module answers and the located query refusals), `tests/javascript_hosted_types.rs` (a variable's JSDoc `@type` under both dialects, a later declaration's, types with JavaScript-only meaning, mapped and literal types by their tokens, and JavaScript's type-argument defaults beside TypeScript's), `tests/javascript_commonjs.rs` (a `module.exports = name` module shaped like a web framework's configuration: default-import, member, in-module, and `import("...")` types, references, the module's own `module` variable, constructs that declare nothing, and session refreshes of its `@type` and exports), and `tests/javascript_refusals.rs` with the classifier's unit tests (each refused construct, every other CommonJS form among them, by each route and checking state, dependency tolerance, constructs TypeScript gives no JavaScript-only meaning, each dialect's own refusals and JSDoc ownership, what only 7.0.2 gives meaning, refused in both dialects by each route and from a dependency, and session refreshes, including a dialect change with the JavaScript unchanged). Refusals are this tool's capability limits, so their expected outcome is the refusal itself; the compilers decide which constructs have JavaScript-only meaning. [references.md](references.md#typescripts-javascript-only-branches) maps each `IsInJSFile` site to its test.

Package identity is pinned in Rust at `Project::load` and query results, against both compilers on scratch copies run from their real path (`tsc --explainFiles --listFilesOnly --incremental false`, whose `with packageId` and `File redirects to file` lines show each id and redirect; an assignment such as `const check: Same = "yes"` for each identity probe; declaration emit, `--declaration --emitDeclarationOnly`, for a module named through a redirect; `tools/ts56-oracle.mjs` and `tools/semantic-oracle.mjs` for references). `tests/package_redirects.rs` covers one identity for a class with a protected member and a unique symbol in both dialects and `deduplicatePackages` under 7.0.2, the copy collection meets first in both import orders, peers (different, equal, and absent installed versions, the order and validity rules of each dialect, versionless peer manifests, and a store of linked package directories with and without `preserveSymlinks`), which routes carry an id (relative and `paths` names of a file and of a directory, a substitution with its extension, an `exports` subpath and a `typesVersions` substitution, a type reference, the route that first reaches a file), failures only a redirected copy reaches (a syntax error, an unreadable file, a missing reference, and a lookup that cannot read a `package.json`, also when a file that stays in the program meets the same lookup afterwards), the `name` and `version` each compiler reads (an empty name, repeated fields, a peer's repeated `version`), references through a redirect, the refusal of every request that names a redirected file, provenance where a root's reference reaches a redirected copy (the provenance `tools/program-inputs.mjs` reports with and without `--oracle=5.6`, through an outline of the file only the copy references), 5.6.3 following the target's references from the copy it processes again (a global interface the copy's own `note.d.ts` merges, which `--type` and `--outline` show under 5.6 only, its missing `note.d.ts`, reported unless `skipLibCheck` skips it, an augmentation of that `note.d.ts`, which applies under 5.6 and is TS2664 under 7.0.2, and an unreadable `package.json` that only lookups from the copy's location meet, which fails the load under 5.6 only, ahead of an unreadable input imported earlier), and a session refreshing each input of an id, a copy's directory link retargeted included, with and without `preserveSymlinks`. `tests/reference_loading.rs` pins that a lookup failure is ordered among the inputs' failures by the file that met it. `src/project/program/tests/redirects.rs` checks the program's inputs, its redirects, the rewired request, `FileLookup`'s refusal, a redirect met inside its target's imports, a `types` package that redirects only under 5.6, module names generated through a redirect (5.6.3's `getModuleSpecifiers` and both compilers' declaration emit), each input's provenance where a route without a package lookup reaches a redirected copy, directly, through a dependency file it reaches again, or met through a directory's id under 5.6, and the inputs and redirects 5.6.3's processing of such a copy again adds for each kind of its target's subtasks (a reference path, an import, a type reference) and each route (a root's reference, a dependency file a root reaches again, the copy as a root), with the library reference that adds nothing.

A construct joins `REFUSED_5_6` in `tests/javascript_refusals.rs` when 5.6.3 gives it meaning and 7.0.2 does not, `REFUSED_7` when 7.0.2 gives it meaning and 5.6.3 does not (both dialects refuse it, as both answer through the binder and checker that port 7.0.2 for the supported slice alone), and an admitted case when neither does. Decide meaning from both compilers on a scratch project: the JavaScript file's own declaration emit (`--declaration --emitDeclarationOnly`) shows a member, class, or namespace the binder declared; a TypeScript consumer's emit (`export const v = expression;`) or `tools/ts56-oracle.mjs`'s quick info shows a type a tag gave. Do not read types from TS2322 messages, which widen a literal source (`"x"` prints as `string`). Where emit shows nothing, as for a binder declaration the checker does not merge into a type, read 5.6.3's binder state through its API (`program.getTypeChecker()`, then a symbol's `exports` or `members`); `src/project/binding/javascript/typescript_5_6.rs` names the 5.6.3 function each decision ports.

## Artemis acceptance 2026-10-27

Artemis revision `294b702c63a3a93f2d1bd01b5974e318a1994e86`, with its
unchanged configuration and installed TypeScript **5.6.3**, loads through
the native TypeScript 5.6 dialect. Every requested command family was
exercised. This is a completed comparison with qualified compatibility:
partial answers and refusals below remain real limits, not equivalent
answers. TypeScript 7.0.2 remains the semantic authority; the pinned 5.6.3
compiler governs dialect comparisons. No alternate configuration, stubs,
source exclusions or dependency upgrades were used to make the project
load. Compiler and editing work used disposable clones, with incremental
compilation disabled for compiler programs.

The oracle audit captured 125 commands and 250 stdout/stderr outputs; all
250 output hashes verified. The input inventories and the hashes of all
7,896 inputs plus `tsconfig.json` agreed across the real checkout, a
protected clean copy and the disposable comparison clone: 7,897 checks, zero
mismatches. The real checkout's status stayed empty and its revision
unchanged through the oracle, service and editing audits.

### Inputs, operations and oracle answers

| Comparison | Result |
| --- | --- |
| Unique program inputs | 7,896 in both tools: 4,977 project, 2,845 dependency and 74 standard-library files. Project inputs include three JavaScript files. No missing/extra inputs or provenance differences. |
| Package redirects | All 18 primary/duplicate relationships agree. Native's raw 7,914 SourceFiles include redirects; the tool records them separately from its unique semantic inputs. |
| Program order | 858 unequal indexed rows, caused entirely by four moved files: React global declarations, csstype, React index and jsx-runtime. Removing those four makes the remaining order identical. See the retained predicate difference below. |
| Parse and syntax check | Representative `.ts`, `.tsx` and `.js` inputs succeed. Native syntax diagnostics are empty; top-level spans and source endpoints agree for 4, 9 and 5 statements. Whole-tree numeric-kind serialization identity with 5.6.3 is not claimed. |
| Navigation | Local/project outline and show agree byte for byte. Definitions reach the CommonJS and barrel declarations. The printed import-miss definition command succeeds. Test outline and statement selection return the complete test statement at lines 4–8. Doctor passes seven checks. |
| Lexical references | `useItracingAccessLevel`: 25/25; name and position agree. Calls: 8/8. JavaScript `COLORS`: 140/140, including a renamed import; unrelated local `COLORS`: 2/2. |
| Member references | `AccessTokenSynchronizer.loadToken`: 11/12; `AppConfigSource.getPublicAppConfig`: 5/30; `ITRACING_ACCESS_LEVELS.FULL`: 13/15. No extras. All 39 candidate gaps were classified; 11 are native non-targets and 28 are omitted targets. Causes are recorded below. |
| Reference scope | All 71 `SWRConfig` and 1,151 `fireEvent` project references agree. Native additionally reports two and one dependency occurrences, respectively; the tool's documented project-only occurrence scope excludes those. |
| Types and depth-2 expansion | 19 cases cover hooks, props, DOM, awaited fetch, typed arrays, Jest, Node, aliases, barrels, baseUrl, JavaScript/CommonJS, augmentation and constructor-parameter widening. Seventeen underlying types agree, allowing the explicitly classified presentation/order differences below. Two typed-array queries refuse. Successful expansions have zero issues and identify depth truncation. |
| DOM expansion | All 183 declaration identities survive comparison: 181 direct member-type string matches and two ordering-only differences from distinct `PointerEventsCheck` symbols. Names and optional flags all match. Explicit property-list normalization leaves no unexplained type difference. Component props, callback parameters/returns and hook return literals also agree. |
| Constructions | Four correct `FetchTokenResult` sites, no missed targets. Broad search examines 5,028 candidates with 1,284 gaps: 861 `flow_required`, 256 `jsx_typing_required`, 165 `unsupported_semantic_form`, two `error_type`. A larger native scan of 5,081 candidates confirms all reported gaps are non-targets. File-filtered search returns the same four sites with no gaps. |

### Service, resources and invalidation

Twelve representative queries have byte-identical stdout, stderr and exit
status through direct and service execution: lexical/member references,
filtered calls, JavaScript/CommonJS references, type summaries, expansion,
library-dependent types, outline and show. The existing partial
`flow_required` answer is identical too. Doctor passes before and after;
`--doctor --direct` is outside the CLI's supported flag scope.

| Measurement | Observed result |
| --- | --- |
| First cold query | 8,002.894 ms |
| Twenty ordinary warm samples | 583.115–1,967.427 ms; median 852.583 ms |
| Repeated cold query | 3,134.105 ms |
| Fifteen repeated warm samples | 634.109–805.482 ms; median 673.856 ms |
| After real-project queries | 1,925.703 MiB RSS; 1,807.736 MiB physical footprint |
| Generation 15 after one cold load and fourteen source refreshes | 2,716.891 MiB RSS; 3,126.269 MiB physical footprint |
| After another 20.057 seconds idle | 2,523.594 MiB RSS; 3,126.378 MiB physical footprint |

Memory was measured with macOS `proc_pid_rusage`, including physical
footprint (compressed memory included). The 6,144 MiB configured limit is
not a measurement; status has no memory field. Every one of the fifteen
generations matched a fresh direct load. After a full load the physical
footprint was 1,685–1,808 MiB; refreshes raised it past 3,000 MiB by
generation 10, and from generation 12 through 15 and 20 seconds of idling it
held at 3,123–3,130 MiB (3.28 GB at its last sample). That exceeds the
[operating envelope](../ENVELOPE.md)'s working ceiling of about 2 GB of
retained memory per service, and every warm sample misses its 100–200 ms
target. Load averages were roughly 9–11 on 18 logical CPUs during the
repeated timing sample. No controlled-load baseline or allocation breakdown
establishes product cost, dominant retained structures, an unbounded leak,
or TypeScript 5.6 causation.

Invalidation passed for source edits (25 → 26 references), paths mapping
selection, dependency declarations and versions, installed compiler
manifest changes (5.6.3 → 5.7.3 → 5.6.3), installed library declarations,
a JavaScript value (24000 → 24001), and a nested duplicate-package version
(identity-dependent answer changes from “same” to “different”). Each
service answer equals a fresh direct load. Repeated failed refreshes return
errors for type, references and show without stale answers; repair restores
correct answers. The compiler-manifest mutation proves invalidation, not
compatibility with an actual 5.7 implementation.

### Restricted environment and lock correction

The preserved macOS sandbox transcript demonstrates the full-project
journey: an inaccessible default directory with `TMPDIR` unset returns
`transport.directory`, exit 2, and both variable remedies. A permitted
scratch `TMPDIR` succeeds in 16.10 s. Doctor names the selection rule and
reports 102 of 103 socket-path bytes; status and stop identify and terminate
the same process. Explicit-directory precedence, privacy refusals for
0750/0705/0755, and socket lengths 102/103/104 bytes are covered: `TMPDIR`
is selected at 102 and 103, skipped at 104. The original redirected query
JSON was not retained; the transcript proves successful service operation,
not byte equality of that unavailable response.

Acceptance also reproduced a separate case: an existing readable mode-0700
default directory whose lock writes are denied. It returned `transport.lock`
without a directory remedy, while doctor passed its metadata checks. This
small correction reuses `runtime::Rule::remedy` for all lock failures,
including failures relayed from the child service. The OS cause, lock path,
`transport.lock` identity and exit 2 remain intact. Text prints the `next:`
remedy on stderr; JSON includes `error.remedy` with no stderr.

`tests/runtime_directory.rs` first failed on the missing final CLI remedy,
then passed with the correction. Reverting only the service change again
fails exactly the two lock tests, each on its missing `next:` remedy, while
the other seven pass. Its sandbox cases cover unset, empty,
relative and overlong `TMPDIR`, plus selected `TMPDIR` and explicit
`TSPARSE_SERVICE_DIR`; existing socket tests cover one byte below, at and
above the limit. `tests/service_startup.rs` verifies child-failure relay and
foreground output, including a diagnostic that cannot be written. This is
service product policy, not a compiler rule. Doctor still checks metadata
and socket length rather than lock writability; that separate limit remains.

The runtime tests also cover a child-only lock failure when the client
selected `TMPDIR`: text and JSON must retain the client's remedy, even
though the child receives an explicit runtime directory. Substituting the
child's selection rule makes that regression test fail. Sandbox directory
denials are checked independently for each response, allowing either the
creation or inspection failure with the exact cause and remedy; the test
does not assume the default directory stays present or absent between
requests.

The corrected CLI was also rerun against a fresh disposable Artemis clone.
With `TMPDIR` unset or skipped for a 104-byte socket path, both text and JSON
retain the permission error and exit 2 while naming both remedies and the
selection reason. With a writable `TMPDIR`, all 25 hook references return
and stdout, stderr and exit status match a fresh direct query byte for byte.
Status/stop/status confirm the service was terminated. This follow-up used
an optimized debug build under concurrent validation load; its 25.95 s
service start and 5.42 s direct query are functional evidence, not comparable
release-performance samples.

Finally, a release build of the corrected tree ran the originally failing
service-backed query (`--references --name useItracingAccessLevel --limit
all`) read-only on the real checkout, with a scratch `TMPDIR`. Doctor passed
all seven checks, selecting `TMPDIR/tsparse-UID` with 90 of 103 socket-path
bytes. The query exited 0 with 25 occurrences in 11 files, complete; the
cold and warm service answers equal `--direct` byte for byte, and
status/stop/status confirm the service ended. The checkout's status and
revision were unchanged before and after.

### Editing on a disposable copy

Body replacement, statement insertion and a three-edit batch across two
files were planned, inspected and applied. Direct/service previews agree;
complete output bytes match independently constructed expectations,
preserving comments, declarations and surrounding formatting. Of 8,591
tracked clone entries, only the two intended TypeScript files changed.

Changed contents, a deleted target and replacement by a new inode with
identical bytes all refuse as `Stale`, exit 1, effect unchanged, with no
leftovers. The contents case used project preparation; deletion and inode
replacement used file-local preparation. All exercise the shared apply-time
checks. Overlapping batches refuse without partial writes. JavaScript and
duplicate-package targets refuse in direct/service JSON and text modes;
apply also refuses altered plans naming JavaScript via targets or aliases.

The edited project loads, both edited files pass syntax checks, and the
final direct/service reference answers agree. All baseline 25 and final 29
reference start/end ranges match 5.6.3: four added, none removed. Definition
and write flags agree under a syntax-checked projection for this corpus;
this is not general equivalence with raw compiler flags. The immediate
post-apply byte proof establishes the tool's output; a later stale-content
restoration script wrote the final hook copy, whose equality establishes
preservation separately. The protected source/configuration inventory of
9,569 entries stayed unchanged (excluding dependencies, Git metadata and
build output from that particular inventory). All measurement/editing
services were stopped.

### Remaining issues and resolved audit artifacts

| Item | Cause, scope and disposition |
| --- | --- |
| JSX-runtime admission/order | **Retained compatibility difference.** 5.6.3 `collectExternalModuleReferences` considers JavaScript, and non-declaration files when effective `isolatedModules` is true or the file is an external module. This tool keeps 7.0.2's JSX/TSX-only predicate. React global/csstype/index appear at 658–660 and jsx-runtime at 935, versus native indices 78–81. React has 149 empty interfaces, 133 with bases, but only Event, EventTarget, FormData and Blob cross same-named declarations; their React declarations have neither members nor bases. No contributed member/base/overload sequence changes in this program, and EventTarget definitions are path-sorted. Documentation now states the limit; a port affecting admission, module status, diagnostics and refresh across both dialects is deferred beyond this local correction. No general equivalence is claimed. |
| Installed-compiler filename normalization | **Open harness defect, not compatibility.** `tools/ts56-compiler.mjs::programFileName` shortens every file directly in the selected compiler's lib directory. Selecting Artemis's installed API falsely renames imported `node_modules/typescript/lib/typescript.d.ts`, adding one missing/extra pair and one order row. The pinned-oracle run isolates the artifact and has only the 858 genuine ordering rows. A future regression should preserve dependency paths while shortening standard libraries only. |
| `loadToken` references | **Existing flow coverage.** One omitted native target at `api-manager.ts:100:66` needs schema narrowing at `schemas/util.ts:170:16`. The readonly parameter-property widening regression is resolved; no `overload_recovery` gap remains. |
| `getPublicAppConfig` references | **Existing flow/JSX coverage.** Twenty-five native targets are omitted among 27 candidates: 26 `flow_required` candidates depend on `schemas/util.ts:170:16`; one `jsx_typing_required` candidate has its cause at `pendo-provider.tsx:151:5`. Candidates at `pendo-provider.tsx:93:31` and `pages/_app.tsx:121:43` are native non-targets. |
| `ITRACING_ACCESS_LEVELS.FULL` references | **Existing flow coverage.** Two omitted targets at `use-product-category.ts:159:58` and `itracing/paths.tsx:22:52`, among eleven explicit flow gaps. Causes are schema narrowing at `schemas/util.ts:170:16` and `paths.tsx:23:7`; another non-target candidate depends on `profile-preview.tsx:180:12`. Nine candidates are native non-targets. |
| Typed array at `delivery.ts:85:9` | **Existing flow coverage.** Native returns `Uint8Array`; summary/expansion refuse `flow_required` at 85:38 because evaluating the initializer requires narrowing its match result. An independent typed-array control agrees, excluding missing libraries as the cause. |
| Typed array at `prepare-report.spec.ts:507:11` | **Existing error recovery boundary.** Native returns `Uint8Array`; the tool refuses a consumed `error_type` at `sharp/dist/index.d.mts:665:63`. Sharp's generic Buffer annotation conflicts with installed @types/node 20.10.0's nongeneric Buffer. Neither compiler selects its only alternate `typesVersions` branch (≤4.8). Native recovers and continues; no incorrect compatibility input was found. |
| SWRConfig union and DOM member ordering | **Retained TS7 checker ordering.** The same SWRConfiguration/callback alternatives print in reversed order. Both distinct `PointerEventsCheck` mapped types print numeric properties 1,2 instead of native 2,1: ApiLevel declares Trigger=2 before Call=1, and 5.6 creation/type-id ordering differs from 7's stable type ordering. All constituents and compared structures agree. |
| QuickInfo labels | **Presentation distinction.** Qualified `jest.Expect`/`NodeJS.Process` and a call-specialized matcher signature differ from queried underlying types `Expect`/`Process` and the generic matcher. Supplemental `getTypeAtLocation` agrees; no ambient package is missing. |
| Broad construction gaps | **Existing checker coverage.** All 1,284 gaps are native non-targets: 861 `flow_required`, 256 `jsx_typing_required`, 165 `unsupported_semantic_form` and two `error_type`. Filtering returns the same four target sites without gaps. No missing 5.6 inputs caused these refusals. |
| Flow and JSX coverage | **Pre-existing checker limits, kept.** Every `flow_required` and `jsx_typing_required` gap above comes from operations the checker does not implement yet ([types design](types-design.md): flow narrowing and JSX typing). The flow and JSX member-reference gaps have the counts and causes an earlier probe of this project recorded before 5.6 compatibility existed. All of them are reported as partial answers, not as matches. Implementing either operation is checker work beyond this acceptance. |
| Construction/type failure-reason propagation | **Open recovery-path investigation.** At `svg-utils.ts:309:3`, direct type reports `flow_required` from `schemas/util.ts:170:16`; constructions reports `error_type` during contextual recovery with the schema cause included. Native gives `OrganizationPositionMap \| undefined`; the literal is a non-target and there is no invalid-access diagnostic. Preserve this distinction until a focused synthetic reproduction establishes the intended reason. |
| Retained service memory | **Open, cause unknown.** Physical footprint reaches 3,126 MiB (RSS 2,717 MiB) after one load and fourteen refreshes, and stays there after 20 seconds idle (3,126 MiB, 3.28 GB; RSS 2,524 MiB). The envelope's working ceiling is about 2 GB; the configured limit of 6,144 MiB is not reached. Direct and service answers still agree exactly. No allocation profile exists, so neither the dominant structures nor a TypeScript 5.6 cause is known. Compare a TS7 project of similar size and profile retained allocations before choosing a correction; that work is larger than this acceptance's small fixes. |
| Warm latency | **Open, cause unknown.** Above target in both sample sets. A controlled-load baseline is needed to separate contention from product cost. |
| Doctor writability | **Open metadata limitation.** Mode/ownership and socket-length checks can pass when sandbox policy denies lock writes. The lock remedy is corrected; doctor does not attempt a lock write. |
| Editing harness runtime | **Open pre-existing harness issue.** `tools/editing-flows.mjs` hardcodes `/tmp`; the acceptance runner honored supplied `TMPDIR` and stopped its service. A follow-up should use `os.tmpdir()` and test under restrictions. |
| Installed skill cache | **External environment maintenance.** Cached v0.1.3 still excluded project JavaScript. Current repository guidance describes the supported slice and editing refusals; refreshing the global installation remains separate. |
| Redirect representation and dependency occurrences | **Intentional scope/normalization.** Eighteen redirect SourceFiles become separate redirect records; dependency reference occurrences are excluded. No identity or project occurrence is lost. |
| Ancestor @types layout | **Resolved environmental difference.** A mirrored layout with two roots containing the same 83 package names selects the same inputs/redirects as the standalone clone; nearer packages satisfy duplicate names. |
| Temporary DOM comparator | **Resolved audit defect.** Display-name keys collapsed 183 identities into 178. The corrected comparator preserves declaration origins and proves 181 direct matches plus two ordering differences, superseding 182 plus one. |
| Editing expectation and setup artifacts | **Resolved, no product defect.** The initial insertion expectation omitted the documented blank separator and was corrected before apply. Bash replaced a denied zsh heredoc operation. The manifest reader hashes symlink spellings rather than reading directory targets. The final report includes all three stale states and their preparation modes. |
| Restricted response retention | **Evidence limitation.** The original successful sandbox query's redirected JSON is unavailable; only the preserved transcript is claimed. Future evidence should retain the full response and hash. That exact-profile run was made from an unrestricted shell: inside an already sandboxed runner, a nested `sandbox-exec` exits 71 (`sandbox_apply: Operation not permitted`) before `tsparse` starts, which is a limit of that environment, not a product failure. |
| Refused-construct probe | **Resolved audit artifact.** The first failed-refresh probe assigned `exports.name = 1` inside an existing ES module and expected a refusal; both entries accepted it, correctly. 5.6.3's `setCommonJsModuleIndicator` returns false for a file with explicit module syntax, so the file stays an ES module. The failed-refresh evidence uses a JSDoc `@param` tag instead, which the tool refuses as `Unsupported`. |

### Regression checks and distribution surfaces

Full-scope format, lint and the full registered semantic check pass. Rust
and semantic validation ran sequentially. The full registered harness passes
all 1,148 tests, with zero skips. The full registered Rust suite passes. Three tests
cannot run because this macOS filesystem rejects non-UTF-8 names:

- `a_saved_plan_whose_path_is_not_unicode_is_named_lossily_without_a_command`
- `a_working_directory_that_is_not_unicode_is_unsupported_input`
- `an_included_file_without_a_unicode_name_is_unsupported`

Two maintenance entry points are intentionally ignored by the ordinary
suite: `list_configured_program` needs explicit configuration/output paths,
and `write_reference` writes generated documentation. The latter was run
explicitly to regenerate the CLI reference.

The frozen application-corpus journeys and load baseline were not run:
that external corpus is not present on this machine. This is a missing
corpus, not a passing journey or a skipped compiler comparison.

The agent surfaces were checked for behavior clauses against their code
owners, in addition to coverage of all 16 operations:

| Surface | Implementation spot-check |
| --- | --- |
| `COMMANDS.md` | `compatibility::select`/`describe` and `doctor::compatibility`: any 5.x selects 5.6, only 5.6.3 is validated, and doctor reports compiler identity and library source. Configuration's 5.6 owner supplies `Config` versus `Unsupported` refusals; `Program::file_lookup` supplies the duplicate-copy refusal and primary-file remedy. Namespace diagnostics, import types, widening, JavaScript, selectors, caps and editing were also checked. |
| `plugins/tsparse/skills/tsparse/SKILL.md` | The same compatibility/refusal owners, JavaScript binding rules and editing policy match its detailed workflow guidance. `runtime::Rule::remedy` and `Endpoint::lock_error` match its lock guidance. Its cost paragraph is self-contained for installed users and makes no latency guarantee. |
| `SHARED_AGENTS_BLOCK.md` | Its operation summaries match the registry; compiler selection, configuration refusals, duplicate-copy behavior and directory/lock/privacy remedies match the same owners. |
| `docs/cli-reference.md` | Regenerated from `src/cli/registry.rs` and the failure-code registry. Doctor's compatibility details and `transport.lock`'s remedy description agree with the implementation; the generation drift test checks the complete reference. |
| Plugin metadata and Codex package | Both plugins report 0.4.3 and the compatibility scope. Regeneration with the hook's `skill-sync` command produces matching skill bodies, with only generated frontmatter wrapping, and matching metadata. |

The registry's `write_reference` generates `docs/cli-reference.md` only;
`COMMANDS.md` is handwritten and was checked and updated separately.
Directory creation/inspection and lock failures name directory-selection
remedies; privacy failures retain permission or ownership remedies such as
`chmod 700`. The JSX-runtime owner's comment now names its pinned TypeScript
7 functions and the deliberately retained 5.6 collection difference.

The bounded lock correction changes no compiler rule, dialect selection,
parser, binder, checker or editing contract. Existing flow, JSX and recovery
limits and larger resource work are recorded rather than expanded here.

## Editing

Source-preserving editing ([editing](editing.md)) is checked by behavior-level
Rust tests of its public operations and command, by compiler probes for the
language rules it relies on, and by flows on a disposable copy of Command
Center.

| Check | Result and scope |
| --- | --- |
| Rust tests | In the latest full Rust run (Rust tests, above), these pass: `tests/editing.rs` (sites of both operations, layout and literal interiors, separators, directive and JSDoc relationships, structure, batches and conflicts, positions across every line break, the grammar budget at, under, and over its limit for the original and both candidate parses, and a 40,000-element flat list), `tests/editing_apply.rs` (written bytes and kept permission mode; refused macOS flags; stale files of every kind, including a second name retargeted; a change found by the check after staging; and failures injected before staging, inside staging before the backup link, at the first and a later installation, at restoration, and at cleanup, including another writer before restoration), `tests/editing_plans.rs`, `tests/editing_project.rs`, and `tests/editing_names.rs` (staging names other files hold are left alone), and `tests/editing_cli.rs` (request validity, text and JSON forms, plans, channels, exit codes, stdin fragments, results and plans only as new files whatever the outcome, hard links and dangling links included, never creating an absent file the request or plan names (each source of names, the project's configuration included) under any spelling the file system takes for it (links, `.`, case, Unicode forms, `ß`/`ss`), and creating distinct spellings where it does not, the names of a plan that cannot be loaded included and nothing written for one that cannot be read or decoded (a field of the wrong type or shape, a field given twice, text cut short), outputs checked in a directory whose new files cannot be removed, an output left in an append-only directory reported as left with the name it matched, a reserved name that cannot be looked up named, a result that cannot be written after applying, after saving a plan, or after a plan fails to load, and service and direct agreement). The editing, apply, and plan tests also passed on Linux (`rust:1-slim`, Rust 1.99) as root and as an ordinary user on 2026-10-09, when they still covered the attribute and access control list preservation removed on 2026-10-23; they have not run on Linux since. [Earlier runs](validation-history.md#editing-rust-tests). |
| Language probes | The tokens that continue an expression statement across a line break, and so need a separating `;`, are exactly `(`, `[`, a template, `+`, `-`, `/`, and `<` for TypeScript 6.0.3's syntax API (which TypeScript 7 shares): `++`, `--`, `!`, `.5`, identifiers, and declarations start a new statement, and no statement start continues a type. Directive retargeting (an old `@ts-expect-error` whose line a fragment's directive takes over is reported unused, TS2578, by the native 7.0.2 compiler) and JSX text that begins with a line break come from the round-1 reviews' native-compiler probes (historical review probes are available in Git history). |

### Editing on Command Center

`tools/editing-flows.mjs` runs a scenario of edit batches on a disposable copy:
for each flow, a JSON preview through the service saving a plan, the same
preview with `--direct` (which must be the same document), a text preview,
applying the plan (which must replace exactly the previewed files, each then
holding its earlier bytes with the preview's changes), the native compiler's
`--noCheck` diagnostics of each changed file (which must gain no code),
follow-up queries, and applying the plan again (which must fail as stale).

2026-10-09, after the round-2 review fixes, Command Center `7706073d2` (a
shared clone with the checkout's `node_modules` linked and its generated
`build-info.generated.ts` and `.next/types` copied in), release build: four
flows, five files, no failure, with the native 7.0.2 compiler executed on
each changed file before and after its edit and reporting no new diagnostic
code. Under a load average of about 7, that run took 1.2–2.0 s for warm
previews, 23–63 s for direct previews, and 37–179 ms to apply. The timings
below are from the 2026-10-08 run on an otherwise idle machine, whose native
compiler check did not run ([earlier runs](validation-history.md#editing-on-command-center)).
The scenario is maintained as
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

## Performance and memory

Reference throughput measurements below predate the design fixes. They use an Intel Core i9-9880H at 2.30 GHz, macOS 15.7.7, Rust 1.96, thin LTO, and one codegen unit. Corpus results are medians of three runs. Sources are read before timing; measurements include parsing and AST disposal, excluding JSON generation and file I/O.

| Corpus | Files | Input bytes | MiB/s |
| --- | ---: | ---: | ---: |
| TypeScript compiler source | 77 | 9,400,075 | 39.46 |
| TypeScript 7 standard declarations | 108 | 3,785,246 | 63.80 |
| Application TypeScript/TSX | 27 | 266,669 | 28.79 |

The repeated typed-function benchmark measured medians of 16.32 MiB/s at 76 bytes, 20.67 MiB/s at 76 KB, and 15.87 MiB/s at 760 KB. These are parser measurements, not comparisons to whole-program compilation.

Allocation accounting runs in a separate benchmark so it does not affect throughput. Earlier measurements of the typed-kind and await-summary design changes found that a 76 KB input produces 21,002 arena nodes, makes 11,018 allocation/reallocation calls, retains 2,944,828 requested heap bytes, and peaks at 4,576,657 bytes, matching that audit's baseline. Those changes do not increase retained AST allocations. The 3,151,774-byte compiler checker retains 41,125,851 bytes in both measured versions when using the same absolute filename; peak requested heap rises from 62,837,508 to 62,905,099 bytes (0.11%) for the transient failed-probe cache. Accounting excludes input storage, allocator overhead, JSON buffers, and process RSS.

Interleaved throughput measurements against audit baseline `e2fb418` varied substantially under concurrent host load, so they do not establish a general speedup or regression. The targeted resource improvement is clear: the 156-byte nested-assignment input that exceeded a three-second timeout completes in about 8 ms, including process startup. Failed arrow and function-type interpretations are cached by source position, grammar context, and remaining depth budget; a 4,096-layer function type returns a diagnostic in about 6 ms. Deep class heritage now produces a nesting diagnostic on both CLI and ordinary Rust worker-thread stacks. The default depth remains 256; applications that raise it must provide a correspondingly larger stack, such as `ParseOptions::DEEP_MAX_DEPTH` on a thread of `DEEP_PARSE_STACK_BYTES`. Speculative interpretations count toward the budget, and exceeding it always returns the depth diagnostic.

Reproduce with:

```sh
cargo bench --offline --bench parse
cargo bench --offline --bench parse -- --corpus /path/to/typescript/src/compiler
cargo bench --offline --bench parse -- --corpus /path/to/typescript-7/lib
cargo bench --offline --bench parse -- --corpus /path/to/application/src
cargo bench --offline --bench memory
cargo bench --offline --bench memory -- /path/to/sources
```

### Reference indexing

`cargo bench --offline --bench references [-- path/to/tsconfig.json]` measures project construction, position queries, and requested heap separately from the parser benchmarks. Construction includes reading, parsing, binding, and indexing every program input; queries sample word starts across the sources, at one-based UTF-16 columns. The benchmark counts queries by outcome (groups returned, empty result for keywords and punctuation, and each query error kind) and prints the mean time over all queries and, separately, over those that returned groups. The position-query times below predate that split and are means over all sampled positions. Measured on the same machine as above, release build, after the final review fixes and while another process was running:

| Project | Inputs | Load (median of 5) | Position query | Peak heap during load | Retained after load |
| --- | --- | ---: | ---: | ---: | ---: |
| Generated (300 modules through one barrel) | 302 files, about 150 KB | 73 ms | 2.3 µs | 7.4 MB | 2.1 MB |
| TypeScript 6.0.3 compiler sources | 43 files, 9.4 MB | 756 ms | 132 µs | 142 MB | 47 MB |

Parsing accounts for roughly a third of the compiler's load time at the throughput above. Retained memory is the captured source text plus one record per identifier token, with names interned. Query time grows with the size of the returned groups; the compiler sample includes groups with thousands of occurrences. No persistent cache was added. Loading later began capturing inputs on worker threads, after Command Center measurements showed a single-threaded load ([Command Center performance](performance-command-center.md)).

### Project navigation

`cargo bench --offline --bench navigation [-- path/to/tsconfig.json]` measures what navigation adds to the snapshot: loads, requested heap, and outline and show latency on a small and the largest searched file. [Navigation storage](navigation-performance.md) records the comparison of eager complete records with the compact selection index the snapshot keeps (2026-10-07). On unchanged Command Center, the index adds about 25 MiB (+8%) to the 309 MiB the reference snapshot retains, where eager records added 181 MiB; outlining the largest searched file, which projects its captured text again, takes about 30 ms. [Command Center performance](performance-command-center.md#current-state-2026-10-08) records the end-to-end costs on the unchanged configuration at plan step 6 (2026-10-08): file-only queries in 5–91 ms, cold loads of 8.8–9.2 s, warm service queries of 0.40–0.51 s, over 90% of which is freshness validation, and refreshes of about 10 s.

## Scope

The library requires complete input and returns the first syntax error. Diagnostics carry a message and byte span; they do not reproduce every compiler diagnostic code or recovery tree. Type checking, binding, JSDoc trees, and reference/comment metadata are outside the syntax projection. The recursive grammar budget defaults to 256 and can be configured through `ParseOptions`. It counts grammar entries and context callbacks, rather than AST levels. Long member and binary chains use an arena and iterative JSON traversal.

Passing these corpora establishes broad compatibility; it does not prove correctness for every possible input. The deterministic robustness runner additionally checks truncations, character mutations, deep nesting, large tokens, and long chains for crashes or hangs, with a three-second timeout per process. It is a resource-behavior test, not an acceptance oracle.

Reports are saved under `.cache/`, including `upstream-corpus.json` (or a supplied `--report` path), `upstream-native.json`, `source-corpus.json`, `grammar-corpus.json`, `native-conformance.json`, and `robustness-corpus.json`. Failing sources are preserved for reproduction. The tools return a nonzero status for observed differences, process failures, empty comparisons, missing corpus layouts, unknown options to the API runners, invalid explicit limits, or failed compiler setup. API runners require version 6.0.3. Native runners verify a successful TypeScript 7 version response and record it; global diagnostics, unrelated-file diagnostics in batch checks, and unexplained nonzero exits fail validation. Only the Rust CLI's normal exit 1 counts as a syntax rejection; exit 2, signals, spawn errors, output overflow, and timeouts fail even for compiler-invalid inputs.

## Regenerating identifier tables

`unicode-id-start` is pinned to 1.5.0. Generated exclusions align its Unicode 18 tables with the compiler's identifier rules. To regenerate, first run the exhaustive native check and dump the unmodified Rust dependency classification:

```sh
TYPE_SCRIPT_API=/path/to/typescript/lib/typescript.js \
  TYPE_SCRIPT_NATIVE=/path/to/typescript-7/tsc \
  node tools/unicode-corpus.mjs
cargo run --offline --example identifier_classification
node tools/generate-identifier-exclusions.mjs
cargo fmt --all
cargo test --offline
```

The generator requires complete classifications and refuses a dependency that lacks a compiler-supported identifier character. Generated ranges and their native compiler version are recorded in `fixtures/identifier-ranges.json`.

### Navigation and construction compatibility

`tests/compatibility_operations.rs` exercises definitions, executable import
hints, construction sites, test outlines and statement selection together on
a synthetic 5.6 project with `baseUrl` and `paths`. It compares direct and
service answers, covers JavaScript `.js`, `.jsx`, `.mjs` and `.cjs` inputs in
both dialects, statement-edit refusals, load-time capability refusals, TS1540,
and requests naming package redirects. Definition locations and contextual
properties come from 5.6.3's language service and checker on scratch projects
with incremental compilation disabled. Test outlines and statement selection
are product contracts, with no compiler operation to compare.

Import hints name an existing specifier's definition command. Inverse module
naming is checked separately by
`module_spelling_names_targets_from_base_url_under_5_6` against
`tools/ts56-oracle.mjs`'s `specifier` query; the same captured naming
environment supplies all semantic presentations.

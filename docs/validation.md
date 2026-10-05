# Validation results

Validation uses TypeScript compiler API 6.0.3 for the JSON syntax projection and the native TypeScript 7.0.2 compiler for language acceptance. No differences remain in the checked corpora. The harnesses and authored fixtures are included; compiler installations and external source trees are not distributed with this repository.

| Check | Result and scope |
| --- | --- |
| Rust tests | 49 tests pass on Rust 1.96 and the minimum supported Rust 1.85: one exhaustive identifier test, 30 existing parser tests, six parser regressions, four literal tests, seven CLI tests, and one doc test. New coverage checks nested `for` declaration and expression contexts, committed syntax/depth diagnostics, malformed hexadecimal escapes, tagged-template cooking and flags, unpaired surrogate preservation, and closed stdout for help/version output. Existing coverage includes deep syntax on ordinary worker-thread stacks, UTF-16 offsets, and iterative serialization of a 10,000-operator chain. |
| Validation-runner tests | 20 Node tests pass with the real compiler API available and no skips. They cover JSON container shape, array length, missing properties, deterministic comparison, deep traversal, process failures, shared syntax eligibility, native setup/diagnostic failures, invalid limits, empty selections, and invalid fixtures run through failing executables. |
| Formatting and lint | `cargo fmt --all -- --check` and Clippy with `--all-targets --no-deps -- -D warnings` pass. |
| Authored AST comparisons | 105/105 cases match the compiler API projection, including positions, context flags, and scanner hints. |
| Authored native checks | 168/168 positive and negative cases match TypeScript 7.0.2. |
| Upstream native corpus | 14,935/14,935 virtual files pass the acceptance comparison, including 792 invalid cases; 7,574,481 source bytes. |
| Upstream API corpus | 14,920/14,920 cases pass, including 14,143 full syntax-tree comparisons and 777 invalid-input rejection checks; 7,568,169 source bytes. Fifteen units using syntax removed in TypeScript 7 are excluded from this TypeScript 6 API oracle and included in the native sweep. |
| Generated grammar combinations | 818/818 cases pass, including 79 invalid combinations. |
| Standard declarations | All 108 `.d.ts` files shipped with TypeScript 7.0.2 parse successfully; 3,785,246 bytes. |
| Compiler/declaration AST sweep | 185/185 files match positions, flags, and scanner hints; 13,185,321 bytes. This includes 77 compiler source files and 108 standard declarations. |
| Identifier classification | The exhaustive native baseline covers all 1,114,112 Unicode code points for identifier-start and identifier-part acceptance. The current Rust test checks every code point against the recorded fixture. The updated native harness also passes a 4,096-code-point smoke check in an isolated output directory. |
| Robustness | 3,980 deterministic truncation, mutation, and adversarial cases return successfully or produce a syntax diagnostic, with no crashes or timeouts. Includes deeply nested class heritage, assignment probes, function types, conditional types, mapped types, templates, and other recursive grammar paths at and beyond the nesting budget. |
| Compiler schema | The generated Rust kinds/constants and documented kind table pass the pinned compiler API's regeneration check. |

The upstream corpus is `tests/cases/conformance` and `tests/cases/compiler` at TypeScript 7.0.2 commit `1e4744d68260a7cb91b62b12edc3f6a2187faaf1`. The compiler-source corpus is from TypeScript 6.0.3 commit `050880ce59e30b356b686bd3144efe24f875ebc8`.

The upstream native harness uses `--noCheck --noResolve --noEmit` to isolate syntax. Unquoted `module` declarations and missing operands after `throw` also receive strict grammar checks because the compiler can defer these errors to its checker. Authored tests confirm the corresponding native diagnostics. API runners share one classifier for TypeScript 7's removed syntax, deferred grammar errors, and ignored AMD module-name comments. Source and upstream runners count removed syntax separately and compare eligible invalid inputs as rejections; authored positive fixtures fail when ineligible or invalid. Comparisons omit the compiler's binding data, reference/pragma metadata, JSDoc trees, and lazy caches; see [the JSON contract](ast-json.md) and [normalization code](../tools/ast-comparison.mjs).

## Performance and memory

Reference throughput measurements below predate the design fixes. They use an Intel Core i9-9880H at 2.30 GHz, macOS 15.7.7, Rust 1.96, thin LTO, and one codegen unit. Corpus results are medians of three runs. Sources are read before timing; measurements include parsing and AST disposal, excluding JSON generation and file I/O.

| Corpus | Files | Input bytes | MiB/s |
| --- | ---: | ---: | ---: |
| TypeScript compiler source | 77 | 9,400,075 | 39.46 |
| TypeScript 7 standard declarations | 108 | 3,785,246 | 63.80 |
| Application TypeScript/TSX | 27 | 266,669 | 28.79 |

The repeated typed-function benchmark measured medians of 16.32 MiB/s at 76 bytes, 20.67 MiB/s at 76 KB, and 15.87 MiB/s at 760 KB. These are parser measurements, not comparisons to whole-program compilation.

Allocation accounting runs in a separate benchmark so it does not affect throughput. Earlier measurements of the typed-kind and await-summary design changes found that a 76 KB input produces 21,002 arena nodes, makes 11,018 allocation/reallocation calls, retains 2,944,828 requested heap bytes, and peaks at 4,576,657 bytes, matching that audit's baseline. Those changes do not increase retained AST allocations. The 3,151,774-byte compiler checker retains 41,125,851 bytes in both measured versions when using the same absolute filename; peak requested heap rises from 62,837,508 to 62,905,099 bytes (0.11%) for the transient failed-probe cache. Accounting excludes input storage, allocator overhead, JSON buffers, and process RSS.

Interleaved throughput measurements against audit baseline `e2fb418` varied substantially under concurrent host load, so they do not establish a general speedup or regression. The targeted resource improvement is clear: the 156-byte nested-assignment input that exceeded a three-second timeout completes in about 8 ms, including process startup. Failed arrow and function-type interpretations are cached by source position, grammar context, and remaining depth budget; a 4,096-layer function type returns a diagnostic in about 6 ms. Deep class heritage now produces a nesting diagnostic on both CLI and ordinary Rust worker-thread stacks. The default depth remains 256; applications that raise it must provide a correspondingly larger stack.

Reproduce with:

```sh
cargo bench --offline --bench parse
cargo bench --offline --bench parse -- --corpus /path/to/typescript/src/compiler
cargo bench --offline --bench parse -- --corpus /path/to/typescript-7/lib
cargo bench --offline --bench parse -- --corpus /path/to/application/src
cargo bench --offline --bench memory
cargo bench --offline --bench memory -- /path/to/sources
```

## Scope

The library requires complete input and returns the first syntax error. Diagnostics carry a message and byte span; they do not reproduce every compiler diagnostic code or recovery tree. Type checking, binding, JSDoc trees, and reference/comment metadata are outside the syntax projection. The recursive grammar budget defaults to 256 and can be configured through `ParseOptions`. It counts grammar entries and context callbacks, rather than AST levels. Long member and binary chains use an arena and iterative JSON traversal.

Passing these corpora establishes broad compatibility; it does not prove correctness for every possible input. The deterministic robustness runner additionally checks truncations, character mutations, deep nesting, large tokens, and long chains for crashes or hangs, with a three-second timeout per process. It is a resource-behavior test, not an acceptance oracle.

Reports are saved under `.cache/`, including `upstream-corpus.json` (or a supplied `--report` path), `upstream-native.json`, `source-corpus.json`, `grammar-corpus.json`, `native-conformance.json`, and `robustness-corpus.json`. Failing sources are preserved for reproduction. The tools return a nonzero status for observed differences, process failures, empty comparisons, missing corpus layouts, invalid explicit limits, or failed compiler setup. API runners require version 6.0.3. Native runners verify a successful TypeScript 7 version response and record it; global diagnostics, unrelated-file diagnostics in batch checks, and unexplained nonzero exits fail validation. Only the Rust CLI's normal exit 1 counts as a syntax rejection; exit 2, signals, spawn errors, output overflow, and timeouts fail even for compiler-invalid inputs.

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

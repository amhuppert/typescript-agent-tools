# typescript-parser

A Rust library and `tsparse` CLI for parsing TypeScript and TSX into a TypeScript compiler API shaped JSON AST. The scanner uses **nom 8**. Syntax nodes live in a borrowing arena; JSON output uses an iterative writer so long operator chains do not require a recursive serialization stack. The package has `publish = false`.

The parser targets TypeScript 7, including TSX, deferred imports, import attributes, and explicit resource management. All 14,935 cases in the upstream conformance/compiler corpus pass the native syntax comparison. Eligible syntax trees also match the compiler API projection, including positions, context flags, and scanner hints. See [validation results](docs/validation.md). JSON contains syntax fields and compiler node positions; compiler caches, binding data, JSDoc trees, and comment directives are omitted.

## Build and run

Rust 1.85 or newer is required. Dependencies are included in `vendor/` and selected by `.cargo/config.toml`, so this checkout builds without accessing crates.io:

```sh
cargo build --offline --release
./target/release/tsparse --pretty example.ts
printf 'const answer: number = 42;' | ./target/release/tsparse
./target/release/tsparse --tsx component.tsx -o component.ast.json
./target/release/tsparse --check example.ts
```

Run Cargo from this repository so its dependency overrides apply. If your usual Cargo home is read-only, set `CARGO_HOME="$PWD/.cache/cargo"` before building. Nothing is published or installed globally.

The CLI reads UTF-8 from one input file or stdin (`-`, or no file). `.tsx` and `.jsx` filenames enable JSX automatically; `--tsx` enables it for stdin. `--pretty` indents JSON, `-o` writes a file, and `--check` validates without emitting JSON. Exit codes are **0** for success, **1** for a syntax error, and **2** for usage or I/O errors. Diagnostics use one-based lines and UTF-16 columns.

## Find references

`tsparse --references` loads one `tsconfig.json` project and groups the declarations and occurrences of a symbol, following import and export aliases and keeping shadowed bindings separate:

```sh
./target/release/tsparse --references --project ./tsconfig.json --at 12:8 src/app.ts
./target/release/tsparse --references --project ./tsconfig.json --name run --pretty
```

On macOS and Linux, project queries (references, and outline and show with
`--project`) automatically start a local service and reuse its snapshot across
CLI invocations. Every query checks the captured inputs; changed sources,
configuration, or resolution inputs trigger a full rebuild. A failed refresh
reports its error. Services stop after ten idle minutes. `--direct` answers in
the CLI process instead, with the same output. On Command Center, a Next.js
application whose configuration selects 7,639 program inputs, a load takes
about 9 seconds, a warm query about 0.4 seconds, most of it that check, and a
rebuild after an edit about 10 seconds ([measurements](docs/performance-command-center.md#current-state-2026-10-08)).

```sh
./target/release/tsparse --references-status --project ./tsconfig.json
./target/release/tsparse --references-stop --project ./tsconfig.json
./target/release/tsparse --references-direct --project ./tsconfig.json --name run
```

Use `--references-serve --project CONFIG` for a foreground service managed by a
supervisor. `TSPARSE_SERVICE_DIR` selects a private runtime directory, and
`TSPARSE_SERVICE_IDLE_SECS` sets its positive idle timeout in seconds. Other
platforms use direct loading. The [service design](docs/reference-service-design.md)
describes freshness, process ownership, and the current full-rebuild limitation.

The library exposes immutable snapshots through `typescript_parser::project::Project`
and refreshable generations through `ProjectSession`; reference queries and results are in
`typescript_parser::references`. Program inputs, their order, and their provenance follow
TypeScript 7.0.2's program; on the unchanged Command Center configuration all 7,639 inputs agree
with `tsc --listFilesOnly` ([validation](docs/validation.md#command-center-checkpoint)). Standard
libraries follow `target`, `lib`, `noLib`, `libReplacement`, and `/// <reference lib>` from the
TypeScript 7.0.2 declarations vendored in [`vendor/typescript-7.0.2-lib`](vendor/README.md): their
declarations join groups, but library files are never searched. JSON modules are program inputs
under `resolveJsonModule`, read with the grammar `tsconfig.json` shares; a JSON module's value and
its properties, like other properties and class members, are not reference targets. Module
augmentations merge as TypeScript's checker merges them, so TipTap's augmented `Commands` is one
group with its 78 declarations. Project references are not followed. See
[find references](docs/references.md) for the result format, binding model, exit codes, and
compatibility limits.

## Navigate declarations

`tsparse --outline` lists one TypeScript file's declarations with their kinds, positions,
and one-line signatures; `tsparse --show` prints one declaration's exact source with its
original line numbers:

```sh
./target/release/tsparse --outline src/runs/host.ts
./target/release/tsparse --outline --depth all --json src/runs/host.ts
./target/release/tsparse --show --name createRunHost.startEligible src/runs/host.ts
./target/release/tsparse --show --at 42:10 src/runs/host.ts
./target/release/tsparse --outline --project ./tsconfig.json src/runs/host.ts
./target/release/tsparse --show --project ./tsconfig.json --name createRunHost.startEligible
```

Entries are syntax sites: overloads, merged interfaces, and same-named locals stay
separate, so an ambiguous name lists every candidate's `FILE:LINE:COLUMN` and qualified
selector and exits 1, and `--show --at FILE:LINE:COLUMN` with that location selects one.
`--name a.b` qualifies by the names of containing declarations. As in TypeScript's
workspace symbols, an import or export specifier has a name only where it renames, and a
default import never does, so a name selects
the declaration, not the files that import it. The outline shows two levels unless `--depth`
says otherwise, with one line per import declaration;
`--json` returns one document for every outcome, including errors. Without `--project`,
navigation reads only the named file and parses with the deep budget that project loading
uses. With `--project`, it answers over the files reference queries search, from the same
service and snapshot as references (or in the CLI process with `--direct`); `--show --name`
then needs no file, and JSON names files relative to the configuration's directory, as
reference results do. The library exposes the same operations as
`typescript_parser::navigation`, and over a project's searched files as `Project::outline`
and `Project::show` (also through `ProjectSession::execute`, which refreshes first). See
[navigation](docs/navigation.md) for the catalog rules, output formats, exit codes, and
differences from TypeScript's document symbols; every searched file of Command Center matches
TypeScript's document symbols apart from those documented differences.

## Edit declarations

`tsparse --replace-body` replaces the complete body of a selected function, method,
constructor, accessor, or arrow function, and `tsparse --insert-after` inserts statements
after the statement a selected declaration designates. Selection is `--show`'s, in one file
or, with `--project`, over the project's searched files:

```sh
./target/release/tsparse --replace-body --name createRunHost.startEligible --from body.ts src/runs/host.ts
./target/release/tsparse --insert-after --project ./tsconfig.json --name projectRunHalts \
  --from helpers.ts --plan change.plan.json
./target/release/tsparse --edit-batch edits.json --project ./tsconfig.json --plan change.plan.json
./target/release/tsparse --apply change.plan.json
```

A request prints each selected site and a unified diff of the exact result and writes
nothing; `--plan` saves the prepared batch, and `--apply` writes exactly its bytes after
checking that every file still has its original bytes and identity, restoring every file if
a later one cannot be replaced. Edits are prepared together against the original files and
change only their bodies and insertion seams: the rest of each file stays byte for byte,
inserted lines take the file's indentation and line breaks, and statement terminators are
added where a line break would not separate statements. A prepared edit parses, keeps every
preserved syntax node, and leaves every `@ts-ignore`/`@ts-expect-error` directive and JSDoc
comment governing what it governed; it is not type-checked. The library exposes the same
operations as `typescript_parser::editing`. See [editing](docs/editing.md) for the sites each
operation supports, layout, plans, the filesystem effects of applying, and exit codes.

## Library

```rust
use typescript_parser::{parse, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = "const answer: number = 42;";
    let ast = parse(source)?;
    assert_eq!(ast.root().kind, "SourceFile");

    if let Some(Value::Nodes(statements)) = ast.root().field("statements") {
        for &statement in statements {
            println!("{}: {}", ast.node(statement).kind, ast.text(statement));
        }
    }
    ast.write_json(std::io::stdout().lock(), true)?;
    Ok(())
}
```

`parse_with_options` accepts `ParseOptions { jsx, file_name, max_depth }`; `ParseOptions::for_file(name)` derives them as TypeScript does, with JSX for a `.tsx` or `.jsx` extension in any case. Declaration filenames `.d.ts`, `.d.mts`, `.d.cts`, and arbitrary extensions such as `.d.css.ts`, matched exactly, set the compiler's ambient context. The recursive grammar budget defaults to 256. It counts grammar entries and scoped context callbacks rather than AST levels, including entries made while trying an interpretation that is later discarded, so an input whose final tree would fit can still be rejected. A `<` after an expression is tried as type arguments only when enough `>` tokens remain to close every list the attempt would open, so long flat lists of comparisons such as `f(x < 0, x < 1, …)` stay shallow; one statement that holds dozens of `<` comparisons and as many later `>` tokens can still exceed it, because TypeScript's own attempt nests that deep. Exceeding the budget anywhere returns the `Maximum syntax nesting depth exceeded` diagnostic; the parser never substitutes another interpretation that fits. Raising the budget requires a correspondingly larger stack: `ParseOptions::DEEP_MAX_DEPTH` (1024) is measured to fit a thread of `DEEP_PARSE_STACK_BYTES` (256 MiB), as find references uses for generated declaration files. Parsing requires complete input and returns the first error; there is no recovery tree or type checking.

`Ast` borrows its input; keep the source alive while using it. The arena is immutable: use `source()`, `nodes()`, `root_id()`, `root()`, and `node(id)` to inspect it. Children are `NodeId` links in `Value::Node` or `Value::Nodes` and belong to their originating arena. Rust node kinds are names such as `BinaryExpression`; JSON kinds are numeric `SyntaxKind` values. [The JSON contract](docs/ast-json.md) explains positions, fields, and serialization.

## Syntax coverage

- JavaScript statements, control flow, labels, automatic semicolon insertion, functions, generators, async functions, arrows, classes, private names, decorators, accessors, and static blocks.
- Expressions, operators, optional chains, generic calls and instantiation expressions, assertions, `as`, `satisfies`, regex literals, templates, destructuring, spread, and dynamic imports.
- Type annotations, interfaces, aliases, unions, intersections, conditional and mapped types, `infer`, type queries, indexed access, tuples, template literal types, predicates, function and constructor types, and generic constraints/defaults/variance.
- Enums, namespaces, ambient modules and global augmentation, imports and exports, type-only and deferred imports, `with` attributes, import types, `using`, and `await using`.
- JSX elements and fragments, generic tags, namespaced names, attributes, spread, embedded expressions, and text scanning.

TypeScript 7 uses the TypeScript 6 syntax baseline, with changes such as `namespace` for unquoted namespaces and `with` for import attributes. The parser rejects the removed unquoted `module` declaration, `assert` import attributes, and legacy standalone `?` and `function(...)` JSDoc type spellings. Identifier classification is checked across every Unicode code point against the native compiler. See Microsoft's [TypeScript 7 announcement](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/) and [native compiler repository](https://github.com/microsoft/typescript-go).

## Verification

```sh
cargo test --offline
cargo clippy --offline --all-targets --no-deps -- -D warnings
cargo bench --offline --bench parse
cargo bench --offline --bench parse -- --corpus /path/to/sources
cargo bench --offline --bench memory -- /path/to/sources
node tools/robustness-corpus.mjs

# Supply your installed compiler paths; these tools do not download anything.
TYPE_SCRIPT_API=/path/to/typescript/lib/typescript.js \
  node tools/conformance.mjs --positions --flags --scanner-hints
TYPE_SCRIPT_API=/path/to/typescript/lib/typescript.js \
  node --test tools/*.test.mjs
TYPE_SCRIPT_API=/path/to/typescript/lib/typescript.js \
  node tools/generate-compiler-schema.mjs --check
TYPE_SCRIPT_NATIVE=/path/to/typescript-7/tsc \
  node tools/native-conformance.mjs
node tools/declaration-corpus.mjs /path/to/typescript-7/lib
TYPE_SCRIPT_API=/path/to/typescript/lib/typescript.js \
  node tools/grammar-corpus.mjs --positions --flags --scanner-hints
TYPE_SCRIPT_API=/path/to/typescript/lib/typescript.js \
  node tools/source-corpus.mjs --positions --flags --scanner-hints /path/to/sources
TYPE_SCRIPT_API=/path/to/typescript/lib/typescript.js \
  node tools/upstream-corpus.mjs /path/to/typescript-checkout --positions --flags --scanner-hints
TYPE_SCRIPT_API=/path/to/typescript/lib/typescript.js \
  TYPE_SCRIPT_NATIVE=/path/to/typescript-7/tsc \
  node tools/upstream-native-corpus.mjs /path/to/typescript-checkout
# Editing flows on a disposable copy of a project; they change its files.
TSPARSE=$PWD/target/release/tsparse TYPE_SCRIPT_NATIVE=/path/to/typescript-7/tsc \
  node tools/editing-flows.mjs /path/to/copy/tsconfig.json scenario.json
```

Find references is checked against TypeScript 7.0.2's language service, its semantic oracle, and benchmarked separately:

```sh
export TYPE_SCRIPT_SEMANTIC_API=/path/to/typescript-7.0.2/package
export TYPE_SCRIPT_NATIVE=/path/to/typescript-<platform>-7.0.2/package/lib/tsc
cargo build --offline --example reference_batch
node tools/semantic-conformance.mjs markers               # fixtures/references markers
node tools/semantic-conformance.mjs compare [PATH ...]    # positions, names, and roles
node tools/reference-combinations.mjs --count=400 --out=.cache/reference-combinations
node tools/semantic-conformance.mjs compare .cache/reference-combinations/*
cargo bench --offline --bench references [-- path/to/tsconfig.json]
```

Navigation is checked against TypeScript 7.0.2's document symbols, read from its language server:

```sh
export TYPE_SCRIPT_NATIVE=/path/to/typescript-<platform>-7.0.2/package/lib/tsc
export TYPE_SCRIPT_API=/path/to/typescript-6.0.3/package/lib/typescript.js
node --test tools/navigation-oracle.test.mjs tools/navigation-conformance.test.mjs
cargo build --offline
node tools/navigation-conformance.mjs fixtures/navigation [PATH ...]
```

A real project's program inputs are compared with the compiler's in two steps (see [validation](docs/validation.md#command-center-checkpoint)):

```sh
TSPARSE_PROGRAM_CONFIG=/abs/app/tsconfig.json TSPARSE_PROGRAM_LISTING=/abs/listing.json \
  cargo test --offline --release --lib -- --ignored --exact project::program::tests::list_configured_program
node tools/program-inputs.mjs /abs/listing.json /abs/app/tsconfig.json
```

`scripts/validate/semantic-full.sh` runs the build, `markers`, fixture `compare`, and the `fixtures/navigation` comparison as the `semantic` validation command, finding the compilers in the canonical checkout's `.cache/compiler-oracles` when the variables are unset; see [validation](docs/validation.md#reference-semantics).

`scripts/validate/harness-test-full.sh` runs the Node tests as the `harness-test` validation command. It fills unset compiler variables from the same cache and prints one line with the total, executed, passed, and skipped counts. `scripts/validate/harness-test-changed.sh` runs only the existing `tools/*.test.mjs` files it is given and exits 2 for any other path. With `HARNESS_REQUIRE_COMPILERS=1`, a missing compiler or any skipped test fails the run; see [validation](docs/validation.md#harness-tests).

The API oracle must be TypeScript 6.0.3; the recorded native syntax baseline is TypeScript 7.0.2. Native runners verify a working TypeScript 7 executable and record its version. Optional `TSPARSE` selects another built CLI. The authored fixture, declaration, grammar, and source runners default to `target/debug/tsparse`; upstream and robustness runners default to `target/release/tsparse`. Differential failures and reports are written under ignored `.cache/`. Every Rust parser runner treats only exit 1 as a syntax rejection; usage/I/O errors, launch errors, crashes, and timeouts fail validation. The process tests include invalid fixtures run through failing executables.

API runners share the TypeScript 6-to-7 syntax eligibility and diagnostic rules and one source comparison, which reports facts; each runner decides what they mean. Source and upstream runners count syntax removed in TypeScript 7 separately; eligible invalid inputs are checked for rejection. Authored positive fixtures must be eligible and valid. Corpus paths must select at least one comparison, and explicit `--limit` values must be positive integers. Missing upstream corpus layouts, empty selections, and failed oracle setup return a nonzero status.

MIT license. Third-party dependencies retain their own licenses; see [vendor provenance](vendor/README.md).

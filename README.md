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

`parse_with_options` accepts `ParseOptions { jsx, file_name, max_depth }`. Declaration filenames `.d.ts`, `.d.mts`, `.d.cts`, and arbitrary extensions such as `.d.css.ts` set the compiler's ambient context. The recursive grammar budget defaults to 256 and returns a diagnostic when exceeded. It counts grammar entries and scoped context callbacks, rather than AST levels; raising it requires a correspondingly larger stack. Parsing requires complete input and returns the first error; there is no recovery tree or type checking.

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
  node --test tools/parser-process.test.mjs
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
```

The API oracle is TypeScript 6.0.3; the native syntax oracle is TypeScript 7.0.2. Optional `TSPARSE` selects another built CLI. The authored fixture, declaration, grammar, and source runners default to `target/debug/tsparse`; upstream and robustness runners default to `target/release/tsparse`. Differential failures and reports are written under ignored `.cache/`. Every Rust parser runner treats only exit 1 as a syntax rejection; usage/I/O errors, launch errors, crashes, and timeouts fail validation. The process tests include invalid fixtures run through failing executables.

MIT license. Third-party dependencies retain their own licenses; see [vendor provenance](vendor/README.md).

//! JavaScript syntax: how a `.js`, `.jsx`, `.mjs`, or `.cjs` file parses
//! (`getLanguageVariant` and the parser's JavaScript context), the errors
//! its TypeScript-only syntax gets (`checkJSSyntax`), and the file-local
//! commands that read one. Expected trees are TypeScript 6.0.3's syntax API
//! (`createSourceFile` with the file's name); expected diagnostics are
//! native 7.0.2's (`tsc --pretty false --incremental false`), which 5.6.3
//! matches except where noted.

mod support;

use std::process::{Command, Output, Stdio};
use support::Fixture;
use typescript_parser::{ParseOptions, parse_with_options, source_errors};

/// TypeScript 6.0.3's `NodeFlags.JavaScriptFile`, on every node of a
/// JavaScript file.
const JAVASCRIPT_FILE: u64 = 524_288;

fn json(file: &str, source: &str) -> serde_json::Value {
    let ast = parse_with_options(source, ParseOptions::for_file(file))
        .unwrap_or_else(|error| panic!("{file}: {error}"));
    serde_json::to_value(ast).unwrap()
}

#[test]
fn javascript_parses_with_the_jsx_variant_and_without_expression_type_arguments() {
    for (file, kind) in [
        ("a.js", 1),
        ("a.mjs", 1),
        ("a.cjs", 1),
        ("a.jsx", 2),
        ("A.JS", 1),
    ] {
        let value = json(
            file,
            "f<T>(x);\nconst e = <div/>;\nconst g = (x) => <a>{x}</a>;\n",
        );
        assert_eq!(value["scriptKind"], kind, "{file}");
        assert_eq!(value["languageVariant"], 1, "{file}");
        let statement = &value["statements"][0];
        assert_eq!(
            statement["flags"].as_u64().unwrap() & JAVASCRIPT_FILE,
            JAVASCRIPT_FILE,
            "{file}"
        );
        // `f < T > (x)`: comparisons, not a call with type arguments.
        let expression = &statement["expression"];
        assert_eq!(expression["kind"], 227, "{file}");
        assert_eq!(expression["operatorToken"]["kind"], 32, "{file}");
        assert_eq!(expression["left"]["kind"], 227, "{file}");
        let element = &value["statements"][1]["declarationList"]["declarations"][0]["initializer"];
        assert_eq!(element["kind"], 286, "{file}");
    }
    // A TypeScript file keeps its call, and no JavaScript flag.
    let value = json("a.ts", "f<T>(x);\n");
    assert_eq!(value["statements"][0]["expression"]["kind"], 214);
    assert_eq!(value["statements"][0]["flags"], 0);
    assert_eq!(value["scriptKind"], 3);
    // `new C < T > ()` has no operand inside its parentheses, a syntax
    // error (TypeScript's "Expression expected.").
    let error = parse_with_options("new C<T>();\n", ParseOptions::for_file("a.js")).unwrap_err();
    assert_eq!(error.line_column("new C<T>();\n"), (1, 10));
    assert!(parse_with_options("new C<T>();\n", ParseOptions::for_file("a.ts")).is_ok());
}

/// One file per construct, with the first error native 7.0.2 reports:
/// (line, column, code, message).
const TYPESCRIPT_ONLY: &[(&str, &str, (usize, usize, &str))] = &[
    (
        "annotation.js",
        "let x: number = 1;\n",
        (
            1,
            8,
            "Type annotations can only be used in TypeScript files.",
        ),
    ),
    (
        "optional.js",
        "function f(a?) { return a; }\n",
        (
            1,
            13,
            "The '?' modifier can only be used in TypeScript files.",
        ),
    ),
    // 5.6.3 reports this at the function's name, 1:10.
    (
        "signature.js",
        "function f(a);\nfunction f(a) { return a; }\n",
        (
            1,
            1,
            "Signature declarations can only be used in TypeScript files.",
        ),
    ),
    (
        "importtype.js",
        "import type { A } from \"./other.js\";\n",
        (
            1,
            1,
            "'import type' declarations can only be used in TypeScript files.",
        ),
    ),
    (
        "exporttype.js",
        "const a = 1;\nexport type { a };\n",
        (
            2,
            1,
            "'export type' declarations can only be used in TypeScript files.",
        ),
    ),
    (
        "importspec.js",
        "import { type A } from \"./other.js\";\n",
        (
            1,
            10,
            "'import...type' declarations can only be used in TypeScript files.",
        ),
    ),
    (
        "exportspec.js",
        "const a = 1;\nexport { type a };\n",
        (
            2,
            10,
            "'export...type' declarations can only be used in TypeScript files.",
        ),
    ),
    (
        "importequals.js",
        "import fs = require(\"fs\");\n",
        (1, 1, "'import ... =' can only be used in TypeScript files."),
    ),
    (
        "exportequals.js",
        "const a = 1;\nexport = a;\n",
        (2, 1, "'export =' can only be used in TypeScript files."),
    ),
    (
        "implements.js",
        "class A implements B {}\n",
        (
            1,
            9,
            "'implements' clauses can only be used in TypeScript files.",
        ),
    ),
    (
        "interface.js",
        "interface I { x: number }\n",
        (
            1,
            11,
            "'interface' declarations can only be used in TypeScript files.",
        ),
    ),
    (
        "namespace.js",
        "namespace N { }\n",
        (
            1,
            11,
            "'namespace' declarations can only be used in TypeScript files.",
        ),
    ),
    (
        "module.js",
        "module M { }\n",
        (
            1,
            8,
            "'module' declarations can only be used in TypeScript files.",
        ),
    ),
    (
        "alias.js",
        "type T = number;\n",
        (1, 6, "Type aliases can only be used in TypeScript files."),
    ),
    (
        "enum.js",
        "enum E { A }\n",
        (
            1,
            6,
            "'enum' declarations can only be used in TypeScript files.",
        ),
    ),
    (
        "nonnull.js",
        "const a = [1];\nconst b = a[0]!;\n",
        (
            2,
            11,
            "Non-null assertions can only be used in TypeScript files.",
        ),
    ),
    (
        "as.js",
        "const a = 1 as number;\n",
        (
            1,
            16,
            "Type assertion expressions can only be used in TypeScript files.",
        ),
    ),
    (
        "satisfies.js",
        "const a = 1 satisfies number;\n",
        (
            1,
            23,
            "Type satisfaction expressions can only be used in TypeScript files.",
        ),
    ),
    (
        "typeparams.js",
        "function f<T>(x) { return x; }\n",
        (
            1,
            12,
            "Type parameter declarations can only be used in TypeScript files.",
        ),
    ),
    (
        "modifier.js",
        "class A { private x = 1; }\n",
        (
            1,
            11,
            "The 'private' modifier can only be used in TypeScript files.",
        ),
    ),
    (
        "parammod.js",
        "class A { constructor(public x) {} }\n",
        (
            1,
            23,
            "Parameter modifiers can only be used in TypeScript files.",
        ),
    ),
    (
        "typeargs.js",
        "class B {}\nclass A extends B<number> {}\n",
        (
            2,
            19,
            "Type arguments can only be used in TypeScript files.",
        ),
    ),
    (
        "declare.js",
        "declare const x: number;\n",
        (
            1,
            1,
            "The 'declare' modifier can only be used in TypeScript files.",
        ),
    ),
    (
        "abstract.js",
        "abstract class A {}\n",
        (
            1,
            1,
            "The 'abstract' modifier can only be used in TypeScript files.",
        ),
    ),
    (
        "returntype.js",
        "const f = (): number => 1;\n",
        (
            1,
            15,
            "Type annotations can only be used in TypeScript files.",
        ),
    ),
    // 5.6.3 reports the key's type annotation, 1:15.
    (
        "indexsig.js",
        "class A { [k: string]: any }\n",
        (
            1,
            11,
            "Signature declarations can only be used in TypeScript files.",
        ),
    ),
    (
        "fntype.js",
        "let f: (a: number) => void;\n",
        (
            1,
            8,
            "Type annotations can only be used in TypeScript files.",
        ),
    ),
];

#[test]
fn typescript_only_syntax_in_javascript_gets_the_native_diagnostics() {
    for (file, source, (line, column, message)) in TYPESCRIPT_ONLY {
        let ast = parse_with_options(source, ParseOptions::for_file(*file))
            .unwrap_or_else(|error| panic!("{file}: {error}"));
        let errors = source_errors(&ast);
        let first = errors.first().unwrap_or_else(|| panic!("{file}: no error"));
        assert_eq!(first.message, *message, "{file}");
        assert_eq!(first.line_column(source), (*line, *column), "{file}");
        // The same syntax in TypeScript is no such error.
        let ts_name = file.replace(".js", ".ts");
        if let Ok(ast) = parse_with_options(source, ParseOptions::for_file(ts_name.as_str())) {
            assert!(
                source_errors(&ast)
                    .iter()
                    .all(|error| !error.message.contains("TypeScript files")),
                "{ts_name}"
            );
        }
    }
    // Type arguments in an expression are comparisons, which are no error.
    for source in [
        "const f = (a) => a;\nconst y = f<number>(1);\n",
        "export const ok = 1;\n",
    ] {
        let ast = parse_with_options(source, ParseOptions::for_file("a.js")).unwrap();
        assert_eq!(source_errors(&ast), [], "{source}");
    }
}

fn tsparse(fixture: &Fixture, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tsparse"))
        .args(args)
        .current_dir(&fixture.root)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// A small module in each JavaScript extension: a function, a class with a
/// method, and a constant, with JSX where the variant allows it anywhere.
const MODULE: &str = "\
export function shout(word) {
  return word.toUpperCase();
}
export class Greeter {
  greet(name) {
    return <b>{name}</b>;
  }
}
export const count = 3;
";

#[test]
fn file_local_commands_read_every_javascript_extension() {
    let names = ["a.js", "b.jsx", "c.mjs", "d.cjs"];
    let mut files: Vec<(&str, &str)> = names.iter().map(|name| (*name, MODULE)).collect();
    files.push(("bad.js", "let x: number = 1;\n"));
    let fixture = Fixture::new(&files);
    for name in names {
        let output = tsparse(&fixture, &["--check", name]);
        assert_eq!(output.status.code(), Some(0), "{name}: {output:?}");
        assert_eq!(
            text(&output.stdout),
            format!("ok: {name} parses as TypeScript 7 (jsx: on)\n")
        );
        let output = tsparse(&fixture, &["--outline", name]);
        assert_eq!(output.status.code(), Some(0), "{name}: {output:?}");
        assert_eq!(
            text(&output.stdout),
            format!(
                "{}\n\
                 1:17 function export function shout(word)\n\
                 4:14 class export class Greeter\n\
                 \x20 5:3 method greet(name)\n\
                 9:14 variable export const count = 3\n",
                fixture.path(name).display()
            ),
            "{name}"
        );
        let output = tsparse(&fixture, &["--show", "--name", "Greeter.greet", name]);
        assert_eq!(output.status.code(), Some(0), "{name}: {output:?}");
        assert!(
            text(&output.stdout)
                .ends_with("5 | greet(name) {\n6 |     return <b>{name}</b>;\n7 |   }\n"),
            "{name}: {}",
            text(&output.stdout)
        );
    }
    let output = tsparse(&fixture, &["--check", "bad.js"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(
        text(&output.stderr),
        "bad.js:1:8: Type annotations can only be used in TypeScript files.\n"
    );
    // The tree is whole, so an outline still answers.
    let output = tsparse(&fixture, &["--outline", "bad.js"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

/// A project file's TypeScript-only syntax fails loading at the construct
/// whatever the file's checking: unset, false, or true `checkJs`,
/// `// @ts-nocheck`, `noCheck`, and `skipLibCheck` (native 7.0.2 and
/// 5.6.3 report TS8010 under each). A dependency's, which both compilers
/// also report, is tolerated, as this tool reports no dependency's
/// diagnostics.
#[test]
fn typescript_only_syntax_fails_loading_a_project_file_whatever_its_checking() {
    use typescript_parser::analysis::InputErrorKind;
    let config = |flags: &str| {
        format!(
            r#"{{ "compilerOptions": {{ "allowJs": true, {flags} "noEmit": true, "noLib": true, "types": [], "module": "esnext", "moduleResolution": "bundler", "maxNodeModuleJsDepth": 1 }}, "include": ["src"] }}"#
        )
    };
    for flags in [
        "",
        r#""checkJs": false,"#,
        r#""checkJs": true,"#,
        r#""noCheck": true,"#,
        r#""skipLibCheck": true,"#,
    ] {
        for (file, text, line) in [
            ("src/a.js", "let x: number = 1;\nexport {};\n", 1),
            (
                "src/a.js",
                "// @ts-nocheck\nlet x: number = 1;\nexport {};\n",
                2,
            ),
        ] {
            let config = config(flags);
            let fixture = Fixture::new(&[("tsconfig.json", config.as_str()), (file, text)]);
            let error = fixture.try_load().unwrap_err();
            assert_eq!(error.kind, InputErrorKind::Syntax, "{flags}: {error}");
            assert_eq!(
                error.message,
                "Type annotations can only be used in TypeScript files."
            );
            assert_eq!(error.path.as_deref(), Some(fixture.path(file).as_path()));
            let position = error.position.unwrap();
            assert_eq!((position.line, position.column), (line, 8), "{flags}");
        }
    }
    let config = config("");
    let fixture = Fixture::new(&[
        ("tsconfig.json", config.as_str()),
        (
            "node_modules/dep/package.json",
            r#"{ "name": "dep", "main": "index.js" }"#,
        ),
        ("node_modules/dep/index.js", "export const d: number = 1;\n"),
        (
            "src/c.ts",
            "import { d } from \"dep\";\nexport const e = d;\n",
        ),
    ]);
    fixture.load();
}

const SIGNATURE: &str = "Signature declarations can only be used in TypeScript files.";
const ANNOTATION: &str = "Type annotations can only be used in TypeScript files.";

/// Constructs whose first error the dialects place apart, with native
/// 7.0.2's and 5.6.3's (`tsc --pretty false --incremental false`; `None` for
/// no error). TypeScript 7 checks each node its parser finishes
/// (`checkJSSyntax`), not an ambient module's (`parseAmbientExternalModuleDeclaration`);
/// 5.6.3 walks the finished tree (`getJSSyntacticDiagnosticsForFile`),
/// skips what lies under a reported node, reports at `getErrorSpanForNode`
/// (a declaration's name, a constructor's keyword), and starts a node list
/// where the list starts, before any comment.
type Expected = Option<(usize, usize, &'static str)>;
const BY_DIALECT: &[(&str, &str, Expected, Expected)] = &[
    (
        "signature.js",
        "function f(a);\nfunction f(a) { return a; }\n",
        Some((1, 1, SIGNATURE)),
        Some((1, 10, SIGNATURE)),
    ),
    (
        "bodilessexport.js",
        "export function f();\n",
        Some((1, 1, SIGNATURE)),
        Some((1, 17, SIGNATURE)),
    ),
    (
        "defaultsig.js",
        "export default function ();\n",
        Some((1, 1, SIGNATURE)),
        Some((1, 1, SIGNATURE)),
    ),
    (
        "bodilessmethod.js",
        "class A {\n  m();\n  m() {}\n}\n",
        Some((2, 3, SIGNATURE)),
        Some((2, 3, SIGNATURE)),
    ),
    (
        "bodilessctor.js",
        "class A {\n  constructor();\n  constructor() {}\n}\n",
        Some((2, 3, SIGNATURE)),
        Some((2, 3, SIGNATURE)),
    ),
    (
        "indexsig.js",
        "class A { [k: string]: any }\n",
        Some((1, 11, SIGNATURE)),
        Some((1, 15, ANNOTATION)),
    ),
    (
        "indexclass.js",
        "export class C { [key: string]: number; }\n",
        Some((1, 18, SIGNATURE)),
        Some((1, 24, ANNOTATION)),
    ),
    (
        "declarefn.js",
        "declare function f(): void;\n",
        Some((
            1,
            1,
            "The 'declare' modifier can only be used in TypeScript files.",
        )),
        Some((1, 18, SIGNATURE)),
    ),
    (
        "asnested.js",
        "const a = (1! as number);\n",
        Some((
            1,
            12,
            "Non-null assertions can only be used in TypeScript files.",
        )),
        Some((
            1,
            18,
            "Type assertion expressions can only be used in TypeScript files.",
        )),
    ),
    (
        "triviaparams.js",
        "function f< /* c */ T>(x) { return x; }\n",
        Some((
            1,
            21,
            "Type parameter declarations can only be used in TypeScript files.",
        )),
        Some((
            1,
            12,
            "Type parameter declarations can only be used in TypeScript files.",
        )),
    ),
    (
        "triviamods.js",
        "class A { constructor( /* c */ public x) {} }\n",
        Some((
            1,
            32,
            "Parameter modifiers can only be used in TypeScript files.",
        )),
        Some((
            1,
            23,
            "Parameter modifiers can only be used in TypeScript files.",
        )),
    ),
    (
        "global.js",
        "export {};\ndeclare global { }\n",
        None,
        Some((
            2,
            9,
            "'module' declarations can only be used in TypeScript files.",
        )),
    ),
    (
        "ambientmodule.js",
        "declare module \"x\" { }\n",
        None,
        Some((
            1,
            16,
            "'module' declarations can only be used in TypeScript files.",
        )),
    ),
    (
        "constenum.js",
        "const enum E { A }\n",
        Some((
            1,
            12,
            "'enum' declarations can only be used in TypeScript files.",
        )),
        Some((
            1,
            12,
            "'enum' declarations can only be used in TypeScript files.",
        )),
    ),
    (
        "optmethod.js",
        "class A { m?() {} }\n",
        Some((
            1,
            12,
            "The '?' modifier can only be used in TypeScript files.",
        )),
        Some((
            1,
            12,
            "The '?' modifier can only be used in TypeScript files.",
        )),
    ),
];

/// The file-local check reads TypeScript 7's rule; project loading reads the
/// dialect's, for each construct of `TYPESCRIPT_ONLY` (where only the two
/// noted differ) and of `BY_DIALECT`.
#[test]
fn typescript_only_syntax_is_reported_by_each_dialects_rule() {
    use support::Dialect::{TypeScript7, TypeScript56};
    use typescript_parser::analysis::InputErrorKind;
    let config = r#"{ "compilerOptions": { "allowJs": true, "noEmit": true, "noLib": true, "types": [], "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true, "target": "es2017", "alwaysStrict": true }, "include": ["src"] }"#;
    let mut cases: Vec<(&str, &str, Expected, Expected)> = TYPESCRIPT_ONLY
        .iter()
        .filter(|(file, ..)| !matches!(*file, "signature.js" | "indexsig.js"))
        .map(|&(file, source, expected)| (file, source, Some(expected), Some(expected)))
        .collect();
    cases.extend_from_slice(BY_DIALECT);
    for (file, source, seven, five) in cases {
        let ast = parse_with_options(source, ParseOptions::for_file(file)).unwrap();
        let local = source_errors(&ast)
            .first()
            .map(|error| (error.line_column(source), error.message.clone()));
        let expected_local =
            seven.map(|(line, column, message)| ((line, column), message.to_owned()));
        assert_eq!(local, expected_local, "{file}: file-local check");
        for (dialect, expected) in [(TypeScript7, seven), (TypeScript56, five)] {
            let path = format!("src/{file}");
            let fixture =
                Fixture::with_dialect(dialect, &[("tsconfig.json", config), (&path, source)]);
            let result = fixture.try_load();
            let context = format!("{file} under {dialect:?}");
            match expected {
                None => {
                    result.unwrap_or_else(|error| panic!("{context}: {error}"));
                }
                Some((line, column, message)) => {
                    let error = result.err().unwrap_or_else(|| panic!("{context}: loaded"));
                    assert_eq!(error.kind, InputErrorKind::Syntax, "{context}: {error}");
                    assert_eq!(error.message, message, "{context}");
                    let position = error.position.unwrap();
                    assert_eq!(
                        (position.line, position.column),
                        (line, column),
                        "{context}"
                    );
                }
            }
        }
    }
}

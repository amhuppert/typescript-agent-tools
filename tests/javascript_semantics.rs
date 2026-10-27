//! The JavaScript branches TypeScript's checker takes for a plain ES module,
//! at the final outcome, a type query from a TypeScript importer: an object
//! literal without a contextual type is a JavaScript literal
//! (`checkObjectLiteral`), whose missing members read as `any` unless
//! `noImplicitAny` (`isJSLiteralType`); an empty initializer widens to
//! `any` or `any[]` (`widenTypeInferredFromInitializer`); a generic call in
//! JavaScript infers `any` where nothing is inferred (`chooseOverload`'s
//! `AnyDefault`); and a JavaScript object literal method's `this` is the
//! literal (`getContextualThisParameterType`). Expected answers are native
//! 7.0.2's (`tools/type-oracle.mjs`) and 5.6.3's (`tools/ts56-oracle.mjs`),
//! which agree on every one.

mod support;

use support::Fixture;
use typescript_parser::analysis::{AnalysisError, SemanticFailureReason};

const LITERALS: &str = r##"export const PALETTE = { slate: { 300: "#c" }, brand: { main: "#f" } };
export let empty = null;
export let none = undefined;
export let list = [];
export const frozen = Object.freeze({ a: 1 });
export const nothing = Array.of();
export const obj = { a: 1, m() { return this.a; } };
"##;

const USE: &str = r#"import { PALETTE, empty, none, list, frozen, nothing, obj } from "./lit.js";
export const missing = PALETTE.nope;
export const missingNested = PALETTE.slate.nope;
export const elem = PALETTE["zzz"];
export const e1 = empty;
export const n1 = none;
export const l1 = list;
export const f1 = frozen;
export const no = nothing;
export const m1 = obj.m();
"#;

fn config(strict: bool) -> String {
    let strictness = if strict {
        r#""strict": true"#
    } else {
        r#""strict": false, "alwaysStrict": true"#
    };
    format!(
        r#"{{ "compilerOptions": {{ "allowJs": true, {strictness}, "noEmit": true, "lib": ["es2017"], "types": [], "target": "es2017", "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true }}, "include": ["src"] }}"#
    )
}

/// Both compilers' answers, by line of `USE`: `None` for the error type,
/// which this tool reports as a semantic failure.
const LOOSE: [(usize, Option<&str>); 9] = [
    (2, Some("any")),
    (3, Some("any")),
    (4, Some("any")),
    (5, Some("any")),
    (6, Some("any")),
    (7, Some("any[]")),
    (8, Some("Readonly<{ a: 1; }>")),
    (9, Some("any[]")),
    (10, Some("number")),
];

const STRICT: [(usize, Option<&str>); 9] = [
    (2, None),
    (3, None),
    (4, None),
    (5, Some("null")),
    (6, Some("undefined")),
    (7, Some("any[]")),
    (8, Some("Readonly<{ a: 1; }>")),
    (9, Some("any[]")),
    (10, Some("number")),
];

fn check(fixture: &Fixture, expected: &[(usize, Option<&str>)], context: &str) {
    let project = fixture
        .try_load()
        .unwrap_or_else(|error| panic!("{context}: {error}"));
    for &(line, answer) in expected {
        match (fixture.type_at(&project, "src/use.ts", line, 14), answer) {
            (Ok(summary), Some(answer)) => assert_eq!(summary, answer, "{context}: line {line}"),
            (Err(AnalysisError::Semantic(_)), None) => {}
            (result, answer) => panic!("{context}: line {line}: {result:?}, expected {answer:?}"),
        }
    }
}

#[test]
fn plain_javascript_object_literals_initializers_and_calls_match_the_compilers() {
    for (strict, expected) in [(false, &LOOSE), (true, &STRICT)] {
        let config = config(strict);
        let fixture = Fixture::new(&[
            ("tsconfig.json", config.as_str()),
            ("src/lit.js", LITERALS),
            ("src/use.ts", USE),
        ]);
        check(
            &fixture,
            expected,
            &format!("TypeScript 7, strict {strict}"),
        );
    }
}

#[test]
fn plain_javascript_matches_typescript_5_6_with_its_own_libraries() {
    let Some(package) =
        support::typescript_5_6("plain_javascript_matches_typescript_5_6_with_its_own_libraries")
    else {
        return;
    };
    for (strict, expected) in [(false, &LOOSE), (true, &STRICT)] {
        let config = config(strict);
        let fixture = Fixture::new(&[
            ("tsconfig.json", config.as_str()),
            ("src/lit.js", LITERALS),
            ("src/use.ts", USE),
        ]);
        fixture.install_compiler(&package);
        check(
            &fixture,
            expected,
            &format!("TypeScript 5.6, strict {strict}"),
        );
    }
}

/// An untyped JavaScript function, method, or arrow without a contextual
/// type needs no argument (`IsUntypedSignatureInJSFile`, which
/// `getMinArgumentCount` reads), while a contextually typed arrow keeps its
/// context's parameter types: both compilers accept each call, and a
/// two-parameter JavaScript function is assignable to a one-parameter
/// callback, which selects the first overload (`"one"`).
const FUNCTIONS: &str = r#"export function px(n, unit) { return n * 2; }
export const util = { scale(n) { return n * 2; } };
export const doubled = [1, 2].map((x) => x * 2);
export const arrow = (a, b) => "s";
"#;

const CALLS: &str = r#"import { px, util, doubled, arrow } from "./fns.js";
export const p = px(4);
export const s = util.scale();
export const d = doubled;
export const a = arrow();
declare function pick(f: (a: number) => number): "one";
declare function pick(f: (a: number, b: number, c: number) => number): "three";
export const chosen = pick(px);
"#;

#[test]
fn untyped_javascript_signatures_need_no_arguments() {
    let config = config(true);
    let fixture = Fixture::new(&[
        ("tsconfig.json", config.as_str()),
        ("src/fns.js", FUNCTIONS),
        ("src/use.ts", CALLS),
    ]);
    let project = fixture.load();
    for (line, expected) in [
        (2, "number"),
        (3, "number"),
        (4, "number[]"),
        (5, "string"),
        (8, "\"one\""),
    ] {
        assert_eq!(
            fixture.summary_at(&project, "src/use.ts", line, 14),
            expected,
            "line {line}"
        );
    }
}

/// The JavaScript branch of `canHaveSyntheticDefault`: a JavaScript ES
/// module has no synthetic default, so a default import of one without a
/// default export is the error type (TS1192, which native 7.0.2 reports;
/// its type at `d` is the error type), while one without module syntax,
/// such as a script `moduleDetection: force` makes a module, has the
/// synthetic default (`typeof import("./forced")` under 7.0.2), which this
/// tool refuses at the import.
#[test]
fn a_default_import_of_javascript_follows_can_have_synthetic_default() {
    for (detection, file, text, refused) in [
        ("auto", "src/esm.js", "export const a = 1;\n", false),
        ("force", "src/forced.js", "const a = 1;\n", true),
    ] {
        let config = config(true).replace(
            r#""noEmit": true"#,
            &format!(r#""noEmit": true, "moduleDetection": "{detection}""#),
        );
        let specifier = file.trim_start_matches("src/");
        let use_ts = format!("import def from \"./{specifier}\";\nexport const d = def;\n");
        let fixture = Fixture::new(&[
            ("tsconfig.json", config.as_str()),
            (file, text),
            ("src/use.ts", &use_ts),
        ]);
        let project = fixture
            .try_load()
            .unwrap_or_else(|error| panic!("{detection}: {error}"));
        match fixture.type_at(&project, "src/use.ts", 2, 14) {
            Err(AnalysisError::Semantic(error)) => {
                let javascript =
                    error.reason == SemanticFailureReason::JavascriptSemanticsRequired.into();
                assert_eq!(javascript, refused, "{detection}: {error:?}");
                if refused {
                    assert_eq!(
                        error.path.as_deref(),
                        Some(fixture.path("src/use.ts").as_path())
                    );
                }
            }
            other => panic!("{detection}: {other:?}"),
        }
    }
}

/// A JavaScript class extending a generic class without type arguments
/// fills them with `any` (`getInstantiatedConstructorsForTypeArguments`,
/// `getDefaultConstructSignatures`, `getTypeFromClassOrInterfaceReference`;
/// native 7.0.2 answers `any` for `new Sub().v`), which this tool refuses
/// at the heritage clause.
#[test]
fn a_javascript_class_extending_a_generic_class_is_refused_at_its_heritage() {
    let config = config(true);
    let fixture = Fixture::new(&[
        ("tsconfig.json", config.as_str()),
        ("src/box.ts", "export class Box<T> { v!: T; }\n"),
        (
            "src/sub.js",
            "import { Box } from \"./box\";\nexport class Sub extends Box {}\n",
        ),
        (
            "src/use.ts",
            "import { Sub } from \"./sub.js\";\nexport const s = new Sub().v;\n",
        ),
    ]);
    let project = fixture.load();
    match fixture.type_at(&project, "src/use.ts", 2, 14) {
        Err(AnalysisError::Semantic(error)) => {
            assert_eq!(
                error.reason,
                SemanticFailureReason::JavascriptSemanticsRequired.into(),
                "{error:?}"
            );
            assert_eq!(
                error.path.as_deref(),
                Some(fixture.path("src/sub.js").as_path())
            );
            let position = error.position.unwrap();
            assert_eq!((position.line, position.column), (2, 26));
        }
        other => panic!("{other:?}"),
    }
}

/// A JavaScript call no signature accepts infers through overload recovery
/// with `any` defaults (`inferSignatureInstantiationForOverloadFailure`;
/// native 7.0.2 answers `any[]` for `[1, 2].map()` in JavaScript and
/// `unknown[]` in TypeScript). This tool refuses overload recovery in every
/// file kind, at the call.
#[test]
fn a_javascript_call_needing_overload_recovery_is_refused_at_the_call() {
    let config = config(true);
    let fixture = Fixture::new(&[
        ("tsconfig.json", config.as_str()),
        ("src/calls.js", "export const bad = [1, 2].map();\n"),
        (
            "src/use.ts",
            "import { bad } from \"./calls.js\";\nexport const b = bad;\n",
        ),
    ]);
    let project = fixture.load();
    match fixture.type_at(&project, "src/use.ts", 2, 14) {
        Err(AnalysisError::Semantic(error)) => {
            assert_eq!(
                error.path.as_deref(),
                Some(fixture.path("src/calls.js").as_path())
            );
            let position = error.position.unwrap();
            assert_eq!((position.line, position.column), (1, 20), "{error:?}");
        }
        other => panic!("{other:?}"),
    }
}

/// A JavaScript file's JSX element types (`getJsxElementTypeTypeAt`,
/// `getJsxManagedAttributesFromLocatedAttributes`,
/// `getJsxPropsTypeFromClassType`; native 7.0.2 answers `Element` here)
/// are refused at the element, as in every file kind.
#[test]
fn javascript_jsx_element_types_are_refused_at_the_element() {
    let config = config(true).replace(r#""noEmit": true"#, r#""noEmit": true, "jsx": "preserve""#);
    let fixture = Fixture::new(&[
        ("tsconfig.json", config.as_str()),
        (
            "src/jsx.d.ts",
            "declare namespace JSX {\n  interface Element { e: 1 }\n  interface IntrinsicElements { div: {} }\n}\n",
        ),
        ("src/view.jsx", "export const el = <div />;\n"),
        (
            "src/use.ts",
            "import { el } from \"./view.jsx\";\nexport const e = el;\n",
        ),
    ]);
    let project = fixture.load();
    match fixture.type_at(&project, "src/use.ts", 2, 14) {
        Err(AnalysisError::Semantic(error)) => {
            assert_eq!(
                error.reason,
                SemanticFailureReason::JsxTypingRequired.into(),
                "{error:?}"
            );
            assert_eq!(
                error.path.as_deref(),
                Some(fixture.path("src/view.jsx").as_path())
            );
        }
        other => panic!("{other:?}"),
    }
}

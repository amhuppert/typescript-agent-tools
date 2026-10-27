//! A JSDoc `@type` on a JavaScript variable, at the final outcome: a type
//! query. TypeScript 7.0.2's parser reparses the type of a variable
//! statement's or declaration's last comment onto the first declaration
//! without a type (`reparseTags`, `reparseHosted`), and the checker reads
//! it as that declaration's annotation, an `import("...")` type through
//! `getTypeFromImportTypeNode`, whose module the loader follows. 5.6.3
//! reads the same tag as the declared type of the statement's first
//! declaration (`getJSDocType`, `getNextJSDocCommentLocation`) and of a
//! declaration with its own comment, but not as a later declaration's, and
//! also as a function initializer's signature (`getSignatureOfTypeTag`):
//! the TypeScript 5.6 dialect refuses both at the tag. A type TypeScript
//! reads with JavaScript-only meaning (`getIntendedTypeFromJSDocTypeReference`,
//! JSDoc-only type syntax) is refused at that type. Expected answers are
//! native 7.0.2's (`tools/type-oracle.mjs`) and 5.6.3's
//! (`tools/ts56-oracle.mjs`, declaration emit), which agree on every one
//! but the later declarations.

mod support;

use support::Dialect::TypeScript56;
use support::{DIALECTS, Fixture, unsupported_javascript};
use typescript_parser::analysis::{AnalysisError, SemanticFailureReason};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeQuery, TypeRequest};

const CONFIG: &str = r#"{ "compilerOptions": { "allowJs": true, "strict": true, "noEmit": true, "lib": ["es2017"], "types": [], "target": "es2017", "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true }, "include": ["src"] }"#;

/// For refusals, which fail before any library is needed.
const BARE_CONFIG: &str = r#"{ "compilerOptions": { "allowJs": true, "strict": true, "noEmit": true, "noLib": true, "types": [], "target": "es2017", "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true }, "include": ["src"] }"#;

/// Reached only through the JSDoc import types, outside `include`.
const SHAPES: &str =
    "export interface Shape { a: number; b?: string }\nexport type Pair<T> = [T, T];\n";

const VARIABLES: &str = r#"/** @type {number} */
export const n = 1;
/** @type {import("../types/shapes").Shape} */
export const s = { a: 1 };
/** @type {string} */
export const a = "x", b = "y";
export const c = 1, /** @type {number} */ d = 2;
/** @type {Array<string>} */
export let list = [];
/** @type {import("../types/shapes").Pair<number>} */
export var pair = [1, 2];
/** @type {{ k: number }} */
export const { k } = { k: 1 };
/** @type {number} */
/** plain */
export const last = 1;
/** @type {typeof n} */
export const tq = 5;
"#;

const USE: &str = r#"import { n, s, a, b, d, list, pair, k, last, tq } from "./v.js";
export const t1 = n;
export const t2 = s;
export const t3 = a;
export const t4 = b;
export const t5 = d;
export const t7 = list;
export const t8 = pair;
export const t9 = k;
export const t10 = last;
export const t11 = tq;
"#;

/// Both compilers' answers, by line of `USE`.
const ANSWERS: [(usize, &str); 10] = [
    (2, "number"),
    (3, "Shape"),
    (4, "string"),
    (5, "\"y\""),
    (6, "number"),
    (7, "string[]"),
    (8, "Pair<number>"),
    (9, "number"),
    (10, "1"),
    (11, "number"),
];

fn fixture(variables: &str, use_ts: &str) -> Fixture {
    Fixture::new(&[
        ("tsconfig.json", CONFIG),
        ("types/shapes.ts", SHAPES),
        ("src/v.js", variables),
        ("src/use.ts", use_ts),
    ])
}

fn check(fixture: &Fixture, context: &str) {
    let project = fixture
        .try_load()
        .unwrap_or_else(|error| panic!("{context}: {error}"));
    for (line, expected) in ANSWERS {
        assert_eq!(
            fixture.summary_at(&project, "src/use.ts", line, 14),
            expected,
            "{context}: src/use.ts:{line}"
        );
    }
    assert_eq!(
        fixture.summary_at(&project, "src/v.js", 2, 14),
        "number",
        "{context}"
    );
    assert_eq!(
        fixture.summary_at(&project, "src/v.js", 4, 14),
        "Shape",
        "{context}"
    );
}

#[test]
fn a_hosted_type_is_its_variables_declared_type() {
    check(&fixture(VARIABLES, USE), "TypeScript 7");
}

#[test]
fn a_hosted_type_is_its_variables_declared_type_under_typescript_5_6() {
    let Some(package) = support::typescript_5_6(
        "a_hosted_type_is_its_variables_declared_type_under_typescript_5_6",
    ) else {
        return;
    };
    let fixture = fixture(VARIABLES, USE);
    fixture.install_compiler(&package);
    check(&fixture, "TypeScript 5.6");
}

/// TypeScript 7 gives a statement's second `@type` to its second
/// declaration, and the statement's `@type` to the first declaration
/// without its own: `y: number`, `q: string`. 5.6.3 leaves both untyped
/// (`1`, `"b"`), so that dialect refuses each tag. Both type `f` as
/// `(x: number) => number`, 5.6.3 through the arrow's own signature, which
/// that dialect refuses.
const LATER: &str = r#"/** @type {string} @type {number} */
export const x = "a", y = 1;
/** @type {string} */
export const /** @type {number} */ p = 1, q = "b";
/** @type {(x: number) => number} */
export const f = (x) => x;
"#;

const LATER_USE: &str = r#"import { x, y, p, q, f } from "./v.js";
export const t1 = x;
export const t2 = y;
export const t3 = p;
export const t4 = q;
export const t5 = f;
"#;

#[test]
fn a_later_declarations_hosted_type_follows_each_dialect() {
    let fixture = fixture(LATER, LATER_USE);
    let project = fixture.load();
    for (line, expected) in [
        (2, "string"),
        (3, "number"),
        (4, "number"),
        (5, "string"),
        (6, "(x: number) => number"),
    ] {
        assert_eq!(
            fixture.summary_at(&project, "src/use.ts", line, 14),
            expected,
            "src/use.ts:{line}"
        );
    }
    drop(project);
    for (text, at) in [
        (
            "/** @type {string} @type {number} */\nexport const x = \"a\", y = 1;\n",
            (1, 20),
        ),
        (
            "/** @type {string} */\nexport const /** @type {number} */ p = 1, q = \"b\";\n",
            (1, 5),
        ),
        (
            "/** @type {(x: number) => number} */\nexport const f = (x) => x;\n",
            (1, 5),
        ),
    ] {
        let fixture = Fixture::with_dialect(
            TypeScript56,
            &[("tsconfig.json", BARE_CONFIG), ("src/v.js", text)],
        );
        let error = fixture
            .try_load()
            .err()
            .unwrap_or_else(|| panic!("{text:?}: loaded under TypeScript 5.6"));
        fixture.assert_refused(
            &error,
            "src/v.js",
            at,
            &unsupported_javascript("JSDoc @type tag"),
            text,
        );
    }
}

/// A hosted type TypeScript reads with JavaScript-only meaning, refused at
/// that type under both dialects: a name `getIntendedTypeFromJSDocTypeReference`
/// maps (`String` is `string`, not the interface), JSDoc-only type syntax
/// (`?number` is `number | null`), and a type whose syntax needed recovery,
/// such as Closure's `function(number): string`, which 7.0.2's parser does
/// not read as a function type either.
#[test]
fn a_hosted_type_with_javascript_only_meaning_is_refused_at_the_type() {
    for (text, at) in [
        ("/** @type {String} */\nexport const s1 = \"a\";\n", (1, 12)),
        ("/** @type {?number} */\nexport const s2 = null;\n", (1, 12)),
        (
            "/** @type {{ a: Number }} */\nexport const s3 = { a: 1 };\n",
            (1, 17),
        ),
        ("/** @type {*} */\nexport const s4 = 1;\n", (1, 12)),
        (
            "/** @type {function(number): string} */\nexport const s5 = null;\n",
            (1, 12),
        ),
    ] {
        for dialect in DIALECTS {
            let fixture = Fixture::with_dialect(
                dialect,
                &[("tsconfig.json", BARE_CONFIG), ("src/v.js", text)],
            );
            let context = format!("{text:?} {dialect:?}");
            let error = fixture
                .try_load()
                .err()
                .unwrap_or_else(|| panic!("{context}: loaded"));
            fixture.assert_refused(
                &error,
                "src/v.js",
                at,
                &unsupported_javascript("JSDoc type with JavaScript-only meaning"),
                &context,
            );
        }
    }
}

/// `fillMissingTypeArguments` for a JSDoc reference to a generic class or
/// interface (`getTypeFromClassOrInterfaceReference`), whose list is never
/// the error type in JavaScript (`isJavaScriptImplicitAny`): each type
/// parameter past the written arguments takes its default, `any` for a
/// default identical to `unknown` or `{}` or for none, and arguments past
/// the parameters are dropped. A TypeScript file keeps `unknown` and `{}`
/// and makes a list that does not fit the error type. Both compilers give
/// every answer, in a TypeScript importer of the JavaScript constants and of
/// a `module.exports` module's default.
const DEFAULTS: &str = "export interface UBox<T = unknown> { value: T }
export interface EBox<T = {}> { value: T }
export interface NBox<T = number> { value: T }
export interface Plain<T> { value: T }
export interface Two<A, B = A> { a: A; b: B }
export declare class CBox<T = unknown> { value: T }
";

const DEFAULTED: &str = r#"/** @type {import("pkg").UBox} */
export const u = { value: 1 };
/** @type {import("pkg").UBox<string>} */
export const us = { value: "s" };
/** @type {import("pkg").EBox} */
export const e = { value: 1 };
/** @type {import("pkg").NBox} */
export const nb = { value: 1 };
/** @type {import("pkg").Plain} */
export const p = { value: 1 };
/** @type {import("pkg").Two<string>} */
export const t = { a: "a", b: "b" };
/** @type {import("pkg").Plain<number, string>} */
export const many = { value: 1 };
/** @type {Array} */
export const xs = [];
/** @type {Map<string>} */
export const m = new Map();
/** @type {import("pkg").CBox} */
export const c = { value: 1 };
"#;

const DEFAULTED_USE: &str = r#"import { u, us, e, nb, p, t, many, xs, m, c } from "./v.js";
import d from "./cjs.js";
export const j1 = u;
export const j2 = u.value;
export const j3 = us;
export const j4 = e;
export const j5 = e.value;
export const j6 = nb;
export const j7 = p;
export const j8 = t;
export const j9 = many;
export const j10 = xs;
export const j11 = m;
export const j12 = c;
export const j13 = d;
export const j14 = d.value;
export declare const s1: import("pkg").UBox;
export declare const s2: import("pkg").EBox;
export declare const s3: import("pkg").NBox;
export declare const s4: import("pkg").Two<string>;
export const s5 = s1.value;
export declare const s6: import("pkg").Plain;
"#;

fn check_defaults(fixture: &Fixture, context: &str) {
    let project = fixture
        .try_load()
        .unwrap_or_else(|error| panic!("{context}: {error}"));
    for (line, column, expected) in [
        (3, 14, "UBox<any>"),
        (4, 14, "any"),
        (5, 14, "UBox<string>"),
        (6, 14, "EBox<any>"),
        (7, 14, "any"),
        (8, 14, "NBox<number>"),
        (9, 14, "Plain<any>"),
        (10, 14, "Two<string, string>"),
        (11, 14, "Plain<number>"),
        (12, 14, "any[]"),
        (13, 14, "Map<string, any>"),
        (14, 14, "CBox<any>"),
        (15, 14, "UBox<any>"),
        (16, 14, "any"),
        (17, 22, "UBox<unknown>"),
        (18, 22, "EBox<{}>"),
        (19, 22, "NBox<number>"),
        (20, 22, "Two<string, string>"),
        (21, 14, "unknown"),
    ] {
        assert_eq!(
            fixture.summary_at(&project, "src/use.ts", line, column),
            expected,
            "{context}: src/use.ts:{line}:{column}"
        );
    }
    // A TypeScript file's list that does not fit is the error type.
    match project.type_of(TypeRequest::new(TypeQuery::At {
        file: fixture.path("src/use.ts"),
        position: SourcePosition {
            line: 22,
            column: 22,
        },
    })) {
        Err(AnalysisError::Semantic(error)) => assert_eq!(
            error.reason,
            SemanticFailureReason::ErrorType.into(),
            "{context}: {error:?}"
        ),
        other => panic!("{context}: src/use.ts:22: {other:?}"),
    }
}

fn defaults_fixture() -> Fixture {
    Fixture::new(&[
        ("tsconfig.json", CONFIG),
        (
            "node_modules/pkg/package.json",
            r#"{ "name": "pkg", "version": "1.0.0", "types": "index.d.ts" }"#,
        ),
        ("node_modules/pkg/index.d.ts", DEFAULTS),
        ("src/v.js", DEFAULTED),
        (
            "src/cjs.js",
            "/** @type {import(\"pkg\").UBox} */\nconst v = { value: 1 };\nmodule.exports = v;\n",
        ),
        ("src/use.ts", DEFAULTED_USE),
    ])
}

#[test]
fn javascript_type_references_fill_type_arguments_as_typescript_does() {
    check_defaults(&defaults_fixture(), "TypeScript 7");
}

#[test]
fn javascript_type_references_fill_type_arguments_under_typescript_5_6() {
    let Some(package) = support::typescript_5_6(
        "javascript_type_references_fill_type_arguments_under_typescript_5_6",
    ) else {
        return;
    };
    let fixture = defaults_fixture();
    fixture.install_compiler(&package);
    check_defaults(&fixture, "TypeScript 5.6");
}

/// The reparsed type is not written syntax: the public tree of a
/// JavaScript file, in both of its JSON writers, has no annotation on the
/// declaration, as TypeScript's syntax API has none, and the file-local
/// syntax check, which rejects a written annotation in JavaScript, accepts
/// the file (`checkJSSyntax` runs before the reparse).
#[test]
fn a_reparsed_type_is_not_written_syntax() {
    let source =
        "/** @type {import(\"x\").T} */\nexport const v = 1, /** @type {number} */ w = 2;\n";
    let ast = typescript_parser::parse_with_options(
        source,
        typescript_parser::ParseOptions::for_file("a.js"),
    )
    .unwrap();
    let mut written = Vec::new();
    ast.write_json(&mut written, false).unwrap();
    let serialized = serde_json::to_string(&ast).unwrap();
    for json in [String::from_utf8(written).unwrap(), serialized] {
        let tree: serde_json::Value = serde_json::from_str(&json).unwrap();
        let declarations = &tree["statements"][0]["declarationList"]["declarations"];
        for index in 0..2 {
            let declaration = declarations[index].as_object().unwrap();
            assert!(!declaration.contains_key("type"), "{json}");
            assert!(declaration.contains_key("initializer"), "{json}");
        }
    }
    assert_eq!(typescript_parser::source_errors(&ast), []);
    let annotated = typescript_parser::parse_with_options(
        "export const v: number = 1;\n",
        typescript_parser::ParseOptions::for_file("a.js"),
    )
    .unwrap();
    assert_eq!(typescript_parser::source_errors(&annotated).len(), 1);
}

/// A JSDoc type in braces is a mapped type only where its tokens begin one
/// (`nextIsStartOfMappedType`), as in TypeScript; otherwise it is a type
/// literal, an index signature among them. Both compilers give each answer.
#[test]
fn a_hosted_mapped_type_is_one_by_its_tokens() {
    let variables = r#"/** @type {{ [K in "a" | "b"]: number }} */
export const mapped = { a: 1, b: 2 };
/** @type {{ readonly [K in "a"]: number }} */
export const ro = { a: 1 };
/** @type {{ -readonly [K in "a"]: number }} */
export const rw = { a: 1 };
/** @type {{ [k: string]: number }} */
export const index = {};
"#;
    let use_ts = r#"import { mapped, ro, rw, index } from "./v.js";
export const t1 = mapped;
export const t2 = ro;
export const t3 = rw;
export const t4 = index;
"#;
    let fixture = fixture(variables, use_ts);
    let project = fixture.load();
    for (line, expected) in [
        (2, "{ a: number; b: number; }"),
        (3, "{ readonly a: number; }"),
        (4, "{ a: number; }"),
        (5, "{ [k: string]: number; }"),
    ] {
        assert_eq!(
            fixture.summary_at(&project, "src/use.ts", line, 14),
            expected,
            "src/use.ts:{line}"
        );
    }
}

/// `fillMissingTypeArguments` for a JSDoc signature's type parameters: the
/// signature's declaration is in JavaScript (`checkTypeArguments`,
/// `getSignatureInstantiation` with `IsInJSFile(signature.declaration)`), so
/// an explicit, short list of a call, a construct call, or an instantiation
/// expression in TypeScript takes `any` for a `unknown` or `{}` default,
/// while an ordinary default and an explicit argument keep theirs and an
/// inferred call infers as in TypeScript. A signature declared in
/// TypeScript keeps TypeScript's defaults though a JavaScript `@type`
/// names it (`g`, `h`): the declaration decides, not the referencing file.
/// Both compilers give every answer. A short list without a default (`q`)
/// is TS2558, which both recover from in `pickLongestCandidateSignature`
/// with the call's file, TypeScript here, so `unknown`; this tool refuses
/// that overload recovery at the call in every file kind, so the case must
/// not reach the signature's provenance.
#[test]
fn a_javascript_signature_fills_type_arguments_as_typescript_does() {
    let signatures = r#"/** @type {<A, B = {}>(x: A) => B} */
export const e = null;
/** @type {<A, B = number>(x: A) => B} */
export const n = null;
/** @type {<A, B>(x: A) => B} */
export const p = null;
/** @type {new <A, B = unknown>(x: A) => { b: B }} */
export const C = null;
/** @type {import("./sigs").G} */
export const g = null;
/** @type {import("./sigs").H} */
export const h = null;
"#;
    let use_ts = r#"import f from "./sig.js";
import { e, n, p, C, g, h } from "./v.js";
export const r1 = f<string>("x");
export const r2 = f<string, number>("x");
export const g1 = f<string>;
export const r3 = f("x");
export const r4 = e<string>("x");
export const r5 = n<string>("x");
export const r6 = p("x");
export const r7 = new C<string>("x").b;
export const g2 = e<string>;
export const t1 = g<string>("x");
export const t2 = g<string>;
export const t3 = h<string>("x");
export const q = p<string>("x");
"#;
    for dialect in DIALECTS {
        let fixture = Fixture::with_dialect(
            dialect,
            &[
                ("tsconfig.json", CONFIG),
                (
                    "src/sig.js",
                    "/** @type {<A, B = unknown>(x: A) => B} */\nconst f = null;\nmodule.exports = f;\n",
                ),
                ("src/v.js", signatures),
                (
                    "src/sigs.ts",
                    "export type G = <A, B = unknown>(x: A) => B;\nexport interface H { <A, B = {}>(x: A): B }\n",
                ),
                ("src/use.ts", use_ts),
            ],
        );
        if dialect == TypeScript56 {
            let Some(package) = support::typescript_5_6(
                "a_javascript_signature_fills_type_arguments_as_typescript_does",
            ) else {
                continue;
            };
            fixture.install_compiler(&package);
        }
        let project = fixture.load();
        for (line, expected) in [
            (3, "any"),
            (4, "number"),
            (5, "(x: string) => any"),
            (6, "unknown"),
            (7, "any"),
            (8, "number"),
            (9, "unknown"),
            (10, "any"),
            (11, "(x: string) => any"),
            (12, "unknown"),
            (13, "(x: string) => unknown"),
            (14, "{}"),
        ] {
            assert_eq!(
                fixture.summary_at(&project, "src/use.ts", line, 14),
                expected,
                "{dialect:?}: src/use.ts:{line}"
            );
        }
        match project.type_of(TypeRequest::new(TypeQuery::At {
            file: fixture.path("src/use.ts"),
            position: SourcePosition {
                line: 15,
                column: 14,
            },
        })) {
            Err(AnalysisError::Semantic(error)) => {
                assert_eq!(
                    error.reason,
                    SemanticFailureReason::OverloadRecovery.into(),
                    "{error:?}"
                );
                let position = error.position.unwrap();
                assert_eq!((position.line, position.column), (15, 18), "{error:?}");
            }
            other => panic!("{dialect:?}: src/use.ts:15: {other:?}"),
        }
    }
}

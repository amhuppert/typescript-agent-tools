//! The JavaScript this tool refuses, at the final outcome. A refused construct
//! is one to which TypeScript 7.0.2 gives JavaScript-only meaning that this
//! tool does not model: a declaration its binder creates from an assignment
//! or `Object.defineProperty` call (`GetAssignmentDeclarationKind`,
//! `bindModuleExportsAssignment`, `bindExportsOrObjectDefineProperty`,
//! `bindDeferredExpandoAssignment`, `bindThisPropertyAssignment`), a
//! `require` call (`bindCallExpression`, `ForEachDynamicImportOrRequireCall`),
//! a JSDoc tag its parser reparses into syntax (`reparseHosted`,
//! `reparseUnhosted`), or a JSDoc import type its loader follows. Loading
//! fails at the construct, as `Unsupported`, for every project JavaScript
//! file that has one, whatever checks the file; a project file's import of a
//! dependency that has one fails at the import, while a dependency's own
//! import of it is tolerated and binds nothing of it. Constructs to which
//! TypeScript gives no such meaning load, with TypeScript's answers.

mod support;

use support::Dialect::{self, TypeScript7, TypeScript56};
use support::{DIALECTS, Fixture, MANIFEST, unsupported_javascript};
use typescript_parser::analysis::AnalysisError;
use typescript_parser::project::{Project, ProjectRequest, ProjectResponse, ProjectSession};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeAnswer, TypeQuery, TypeRequest};

/// A refused construct: its file's text, where loading fails, and what it is.
struct Refused {
    text: &'static str,
    at: (usize, usize),
    construct: &'static str,
}

const REFUSED: &[Refused] = &[
    Refused {
        text: "module.exports = 1;\n",
        at: (1, 1),
        construct: "CommonJS module.exports assignment",
    },
    Refused {
        text: "module.exports = { a: 1 };\n",
        at: (1, 1),
        construct: "CommonJS module.exports assignment",
    },
    Refused {
        text: "module.exports = class {};\n",
        at: (1, 1),
        construct: "CommonJS module.exports assignment",
    },
    Refused {
        text: "const o = { a: 1 };\nmodule.exports = o.a;\n",
        at: (2, 1),
        construct: "CommonJS module.exports assignment",
    },
    Refused {
        text: "const a = 1;\nmodule.exports = a;\nmodule.exports = a;\n",
        at: (2, 1),
        construct: "CommonJS module.exports assignment",
    },
    Refused {
        text: "const a = 1;\nfunction f() {\n  module.exports = a;\n}\n",
        at: (3, 3),
        construct: "CommonJS module.exports assignment",
    },
    Refused {
        text: "const a = 1;\nvar m = module.exports = a;\n",
        at: (2, 9),
        construct: "CommonJS module.exports assignment",
    },
    Refused {
        text: "const a = 1;\nmodule.exports = a;\nexports.b = 2;\n",
        at: (3, 1),
        construct: "CommonJS exports assignment",
    },
    Refused {
        text: "const a = 1;\nmodule.exports = a;\nmodule.exports.b = 2;\n",
        at: (3, 1),
        construct: "CommonJS exports assignment",
    },
    Refused {
        text: "const a = 1;\nmodule.exports = a;\nObject.defineProperty(exports, \"b\", { value: 2 });\n",
        at: (3, 1),
        construct: "CommonJS Object.defineProperty export",
    },
    Refused {
        text: "const a = require(\"./side.js\");\nmodule.exports = a;\n",
        at: (1, 11),
        construct: "require call",
    },
    Refused {
        text: "const a = <div />;\nmodule.exports = a;\n",
        at: (2, 1),
        construct: "CommonJS module.exports assignment",
    },
    Refused {
        text: "exports.a = 1;\n",
        at: (1, 1),
        construct: "CommonJS exports assignment",
    },
    Refused {
        text: "Object.defineProperty(exports, \"a\", { value: 1 });\n",
        at: (1, 1),
        construct: "CommonJS Object.defineProperty export",
    },
    Refused {
        text: "export function f() {}\nf.x = 1;\n",
        at: (2, 1),
        construct: "property assignment that declares a member of a function, class, or object",
    },
    Refused {
        text: "export const g = () => 1;\ng.y = 2;\n",
        at: (2, 1),
        construct: "property assignment that declares a member of a function, class, or object",
    },
    Refused {
        text: "export class C {}\nC.z = 3;\n",
        at: (2, 1),
        construct: "property assignment that declares a member of a function, class, or object",
    },
    Refused {
        text: "export const o = {};\no.a = 1;\n",
        at: (2, 1),
        construct: "property assignment that declares a member of a function, class, or object",
    },
    // A member through an earlier expando (`getInitializerSymbol`), which
    // both compilers declare, fails at that earlier expando.
    Refused {
        text: "export const o = {};\no.a = {};\no.a.b = 1;\n",
        at: (2, 1),
        construct: "property assignment that declares a member of a function, class, or object",
    },
    Refused {
        text: "export let h = function () {};\nh[\"k\"] = 1;\n",
        at: (2, 1),
        construct: "property assignment that declares a member of a function, class, or object",
    },
    Refused {
        text: "export function d() {}\nObject.defineProperty(d, \"p\", { value: 1 });\n",
        at: (2, 1),
        construct: "Object.defineProperty call that declares a member of a function, class, or object",
    },
    Refused {
        text: "export class T {\n  constructor() {\n    this.a = 1;\n  }\n}\n",
        at: (3, 5),
        construct: "this assignment that declares a class or object literal member",
    },
    Refused {
        text: "export const lit = {\n  m() {\n    this.b = 2;\n  },\n};\n",
        at: (3, 5),
        construct: "this assignment that declares a class or object literal member",
    },
    Refused {
        text: "const fs = require(\"fs\");\nexport { fs };\n",
        at: (1, 12),
        construct: "require call",
    },
    Refused {
        text: "require(\"./side.js\");\nexport {};\n",
        at: (1, 1),
        construct: "require call",
    },
    Refused {
        text: "/** @type {?number} */\nexport const n = null;\n",
        at: (1, 12),
        construct: "JSDoc type with JavaScript-only meaning",
    },
    Refused {
        text: "for (const /** @type {number} */ x of [1]) {}\nexport {};\n",
        at: (1, 16),
        construct: "JSDoc @type tag",
    },
    Refused {
        text: "export const c = /** @type {number} */ (1);\n",
        at: (1, 22),
        construct: "JSDoc @type tag",
    },
    Refused {
        text: "/** @type {(a: number) => void} */\nexport function fs(a) {}\n",
        at: (1, 5),
        construct: "JSDoc @type tag",
    },
    Refused {
        text: "/** @param {string} s */\nexport function p(s) {\n  return s;\n}\n",
        at: (1, 5),
        construct: "JSDoc @param tag",
    },
    Refused {
        text: "/** @arg {string} s */\nexport function p(s) {\n  return s;\n}\n",
        at: (1, 5),
        construct: "JSDoc @arg tag",
    },
    Refused {
        text: "/** @argument {string} s */\nexport function p(s) {\n  return s;\n}\n",
        at: (1, 5),
        construct: "JSDoc @argument tag",
    },
    Refused {
        text: "/** @return {number} */\nexport function r() {\n  return 1;\n}\n",
        at: (1, 5),
        construct: "JSDoc @return tag",
    },
    Refused {
        text: "export class P {\n  /** @type {number} */\n  x;\n}\n",
        at: (2, 7),
        construct: "JSDoc @type tag",
    },
    Refused {
        text: "export function q(/** @type {number} */ a) {\n  return a;\n}\n",
        at: (1, 23),
        construct: "JSDoc @type tag",
    },
    Refused {
        text: "export const g = {\n  /** @type {number} */\n  get v() {\n    return 1;\n  },\n};\n",
        at: (2, 7),
        construct: "JSDoc @type tag",
    },
    Refused {
        text: "export const o = { a: 1 };\n/** @type {number} */\no.a = 2;\n",
        at: (2, 5),
        construct: "JSDoc @type tag",
    },
    Refused {
        text: "export class R {\n  /** @readonly */\n  x = 1;\n}\n",
        at: (2, 7),
        construct: "JSDoc @readonly tag",
    },
    Refused {
        text: "export class U {\n  /** @public */\n  m() {}\n}\n",
        at: (2, 7),
        construct: "JSDoc @public tag",
    },
    Refused {
        text: "export class V {\n  /** @protected */\n  m() {}\n}\n",
        at: (2, 7),
        construct: "JSDoc @protected tag",
    },
    Refused {
        text: "class B {\n  m() {}\n}\nexport class W extends B {\n  /** @override */\n  m() {}\n}\n",
        at: (5, 7),
        construct: "JSDoc @override tag",
    },
    Refused {
        text: "class Base {}\n/** @augments {Base<number>} */\nexport class A2 extends Base {}\n",
        at: (2, 5),
        construct: "JSDoc @augments tag",
    },
    Refused {
        text: "/** @returns {number} */\nexport function r() {\n  return 1;\n}\n",
        at: (1, 5),
        construct: "JSDoc @returns tag",
    },
    Refused {
        text: "/** @template T */\nexport function t(x) {\n  return x;\n}\n",
        at: (1, 5),
        construct: "JSDoc @template tag",
    },
    Refused {
        text: "/** @this {Date} */\nexport function w() {}\n",
        at: (1, 5),
        construct: "JSDoc @this tag",
    },
    Refused {
        text: "/** @satisfies {object} */\nexport const s = {};\n",
        at: (1, 5),
        construct: "JSDoc @satisfies tag",
    },
    Refused {
        text: "export class E {\n  /** @private */\n  m() {}\n}\n",
        at: (2, 7),
        construct: "JSDoc @private tag",
    },
    Refused {
        text: "/** @implements {Iterable<number>} */\nexport class I {}\n",
        at: (1, 5),
        construct: "JSDoc @implements tag",
    },
    Refused {
        text: "class Base {}\n/** @extends {Base<number>} */\nexport class Derived extends Base {}\n",
        at: (2, 5),
        construct: "JSDoc @extends tag",
    },
    Refused {
        text: "/** @typedef {string} Name */\nexport {};\n",
        at: (1, 5),
        construct: "JSDoc @typedef tag",
    },
    Refused {
        text: "/**\n * @callback Fn\n * @param {number} a\n */\nexport {};\n",
        at: (2, 4),
        construct: "JSDoc @callback tag",
    },
    Refused {
        text: "/** @import { X } from \"./x.js\" */\nexport {};\n",
        at: (1, 5),
        construct: "JSDoc @import tag",
    },
    Refused {
        text: "export class O {\n  /**\n   * @overload\n   * @param {number} a\n   */\n  m(a) {}\n}\n",
        at: (3, 6),
        construct: "JSDoc @overload tag",
    },
    Refused {
        text: "/** @param {import(\"./dep.js\").T} a */\nexport const k = 1;\n",
        at: (1, 13),
        construct: "JSDoc import type",
    },
];

const EXPANDO: &str = "property assignment that declares a member of a function, class, or object";

/// What decides whether TypeScript checks a JavaScript file, none of which
/// decides a refusal.
#[derive(Clone, Copy, Debug)]
enum Checking {
    Plain,
    CheckJsFalse,
    CheckJsTrue,
    NoCheckPragma,
    NoCheck,
    SkipLibCheck,
}

const CHECKING: [Checking; 6] = [
    Checking::Plain,
    Checking::CheckJsFalse,
    Checking::CheckJsTrue,
    Checking::NoCheckPragma,
    Checking::NoCheck,
    Checking::SkipLibCheck,
];

fn config(checking: Checking, include: &str, extra: &str) -> String {
    let setting = match checking {
        Checking::CheckJsFalse => r#""checkJs": false,"#,
        Checking::CheckJsTrue => r#""checkJs": true,"#,
        Checking::NoCheck => r#""noCheck": true,"#,
        Checking::SkipLibCheck => r#""skipLibCheck": true,"#,
        Checking::Plain | Checking::NoCheckPragma => "",
    };
    format!(
        r#"{{ "compilerOptions": {{ "allowJs": true, {setting} {extra} "strict": true, "noEmit": true, "noLib": true, "types": [], "target": "es2017", "module": "esnext", "moduleResolution": "bundler" }}, "include": [{include}] }}"#
    )
}

/// The routes by which a project file reaches the refused file `src/lib/a.js`:
/// as a root, through a TypeScript import, and through a JavaScript
/// re-export that a TypeScript file imports.
#[derive(Clone, Copy, Debug)]
enum Route {
    Root,
    Imported,
    ReExported,
}

impl Route {
    fn include(self) -> &'static str {
        match self {
            Route::Root => r#""src""#,
            Route::Imported | Route::ReExported => r#""src/**/*.ts""#,
        }
    }

    fn files(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Route::Root => &[],
            Route::Imported => &[("src/use.ts", "import \"./lib/a.js\";\nexport {};\n")],
            Route::ReExported => &[
                ("src/use.ts", "import \"./lib/b.js\";\nexport {};\n"),
                ("src/lib/b.js", "export * from \"./a.js\";\n"),
            ],
        }
    }
}

/// Every refused construct fails loading at the construct, whichever route
/// makes its file a project file and whatever checks the file.
#[test]
fn a_project_javascript_file_fails_at_its_refused_construct_whatever_checks_it() {
    for route in [Route::Root, Route::Imported, Route::ReExported] {
        for refused in REFUSED {
            for checking in CHECKING {
                let (text, at) = match checking {
                    Checking::NoCheckPragma => (
                        format!("// @ts-nocheck\n{}", refused.text),
                        (refused.at.0 + 1, refused.at.1),
                    ),
                    _ => (refused.text.to_owned(), refused.at),
                };
                let config = config(checking, route.include(), "");
                let mut files = vec![("tsconfig.json", config.as_str()), ("src/lib/a.js", &text)];
                files.extend_from_slice(route.files());
                let fixture = Fixture::new(&files);
                let context = format!("{:?} {route:?} under {checking:?}", refused.text);
                let error = fixture
                    .try_load()
                    .err()
                    .unwrap_or_else(|| panic!("{context}: loaded"));
                let expected = unsupported_javascript(refused.construct);
                fixture.assert_refused(&error, "src/lib/a.js", at, &expected, &context);
            }
        }
    }
}

/// Every construct of `REFUSED` is refused under the TypeScript 5.6 dialect
/// too, at the same place, whatever checks the file: 5.6.3 gives each the
/// same meaning, a variable's `@satisfies` through its initializer
/// (`getContextualTypeForVariableLikeDeclaration`, `checkDeclarationInitializer`).
#[test]
fn refusals_hold_in_the_typescript_5_6_dialect() {
    for refused in REFUSED {
        for checking in CHECKING {
            let (text, at) = match checking {
                Checking::NoCheckPragma => (
                    format!("// @ts-nocheck\n{}", refused.text),
                    (refused.at.0 + 1, refused.at.1),
                ),
                _ => (refused.text.to_owned(), refused.at),
            };
            let config = config(checking, r#""src""#, r#""esModuleInterop": true,"#);
            let fixture = Fixture::with_dialect(
                TypeScript56,
                &[("tsconfig.json", &config), ("src/lib/a.js", &text)],
            );
            let context = format!("{:?} under {checking:?}, TypeScript 5.6", refused.text);
            let error = fixture
                .try_load()
                .err()
                .unwrap_or_else(|| panic!("{context}: loaded"));
            fixture.assert_refused(
                &error,
                "src/lib/a.js",
                at,
                &unsupported_javascript(refused.construct),
                &context,
            );
        }
    }
}

const DEPENDENCY_DEPTH: &str = r#""maxNodeModuleJsDepth": 2,"#;

const DEP_MANIFEST: (&str, &str) = (
    "node_modules/dep/package.json",
    r#"{"name":"dep","main":"index.js"}"#,
);
const DEP_REFUSED: (&str, &str) = (
    "node_modules/dep/index.js",
    "export function v() {}\nv.extra = 2;\n",
);

/// A project file's import of a dependency's refused JavaScript fails at
/// the import, whatever checks the importer, naming the construct.
#[test]
fn a_project_import_of_a_refused_dependency_fails_at_the_import() {
    for checking in CHECKING {
        let config = config(checking, r#""src""#, DEPENDENCY_DEPTH);
        let pragma = match checking {
            Checking::NoCheckPragma => "// @ts-nocheck\n",
            _ => "",
        };
        let importer = format!("{pragma}import {{ v }} from \"dep\";\nexport const w = v;\n");
        let fixture = Fixture::new(&[
            ("tsconfig.json", &config),
            ("src/use.ts", &importer),
            DEP_MANIFEST,
            DEP_REFUSED,
        ]);
        let context = format!("under {checking:?}");
        let error = fixture
            .try_load()
            .err()
            .unwrap_or_else(|| panic!("{context}: loaded"));
        let line = if pragma.is_empty() { 1 } else { 2 };
        let expected = format!(
            "module \"dep\": node_modules/dep/index.js:2:1: {}",
            unsupported_javascript(EXPANDO)
        );
        fixture.assert_refused(&error, "src/use.ts", (line, 19), &expected, &context);
    }
}

/// Every kind of request a project file makes into a dependency's refused
/// JavaScript fails at its specifier: an import, a type-only import, a
/// re-export, a dynamic import, and an import type. A reference directive
/// makes its target a project file, which fails at its construct.
#[test]
fn every_project_request_into_a_refused_dependency_fails() {
    let config = config(Checking::Plain, r#""src""#, DEPENDENCY_DEPTH);
    let expected = format!(
        "module \"dep\": node_modules/dep/index.js:2:1: {}",
        unsupported_javascript(EXPANDO)
    );
    for (importer, at) in [
        ("import { v } from \"dep\";\nexport const w = v;\n", (1, 19)),
        ("import type { v } from \"dep\";\nexport {};\n", (1, 24)),
        ("export { v } from \"dep\";\n", (1, 19)),
        ("export const p = import(\"dep\");\n", (1, 25)),
        ("export type T = typeof import(\"dep\");\n", (1, 31)),
    ] {
        let fixture = Fixture::new(&[
            ("tsconfig.json", &config),
            ("src/use.ts", importer),
            DEP_MANIFEST,
            DEP_REFUSED,
        ]);
        let error = fixture
            .try_load()
            .err()
            .unwrap_or_else(|| panic!("{importer:?}: loaded"));
        fixture.assert_refused(&error, "src/use.ts", at, &expected, importer);
    }
    let fixture = Fixture::new(&[
        ("tsconfig.json", &config),
        (
            "src/use.ts",
            "/// <reference path=\"../node_modules/dep/index.js\" />\nexport const z = 1;\n",
        ),
        DEP_MANIFEST,
        DEP_REFUSED,
    ]);
    let Err(error) = fixture.try_load() else {
        panic!("loaded");
    };
    let expected = unsupported_javascript(EXPANDO);
    fixture.assert_refused(
        &error,
        "node_modules/dep/index.js",
        (2, 1),
        &expected,
        "reference",
    );
}

/// A dependency's own import of refused JavaScript is tolerated: the
/// refused file binds nothing, neither the exports its importer would
/// re-export nor, as a script, globals, while the rest of the dependency
/// keeps its answers.
#[test]
fn a_dependency_import_of_refused_javascript_is_tolerated_and_binds_nothing_of_it() {
    let config = config(Checking::Plain, r#""src""#, DEPENDENCY_DEPTH);
    let fixture = Fixture::new(&[
        ("tsconfig.json", &config),
        (
            "src/use.ts",
            "import { w, v } from \"wrap\";\nexport const a = w;\nexport const b = v;\nexport const c = leaked;\n",
        ),
        (
            "node_modules/wrap/package.json",
            r#"{"name":"wrap","main":"index.js"}"#,
        ),
        (
            "node_modules/wrap/index.js",
            "import \"script\";\nexport { v } from \"dep\";\nexport const w = 1;\n",
        ),
        DEP_MANIFEST,
        DEP_REFUSED,
        (
            "node_modules/script/package.json",
            r#"{"name":"script","main":"index.js"}"#,
        ),
        (
            "node_modules/script/index.js",
            "var leaked = 1;\nrequire(\"x\");\n",
        ),
    ]);
    let project = fixture.load();
    assert_eq!(fixture.summary_at(&project, "src/use.ts", 2, 14), "1");
    for line in [3, 4] {
        match fixture.type_at(&project, "src/use.ts", line, 14) {
            Err(AnalysisError::Semantic(_)) => {}
            other => panic!("src/use.ts:{line}: {other:?}"),
        }
    }
}

/// A dependency's import of any refused construct is tolerated, whatever
/// checks the project or the refused file, and the importer keeps its
/// answers.
#[test]
fn every_refused_construct_is_tolerated_from_a_dependency_importer() {
    for refused in REFUSED {
        for checking in CHECKING {
            let text = match checking {
                Checking::NoCheckPragma => format!("// @ts-nocheck\n{}", refused.text),
                _ => refused.text.to_owned(),
            };
            let config = config(checking, r#""src""#, DEPENDENCY_DEPTH);
            let fixture = Fixture::new(&[
                ("tsconfig.json", &config),
                (
                    "src/use.ts",
                    "import { w } from \"wrap\";\nexport const a = w;\n",
                ),
                (
                    "node_modules/wrap/package.json",
                    r#"{"name":"wrap","main":"index.js"}"#,
                ),
                (
                    "node_modules/wrap/index.js",
                    "import \"dep\";\nexport const w = 1;\n",
                ),
                DEP_MANIFEST,
                ("node_modules/dep/index.js", &text),
            ]);
            let context = format!("{:?} under {checking:?}", refused.text);
            let project = fixture
                .try_load()
                .unwrap_or_else(|error| panic!("{context}: {error}"));
            assert_eq!(
                fixture.summary_at(&project, "src/use.ts", 2, 14),
                "1",
                "{context}"
            );
        }
    }
}

/// Constructs to which TypeScript 7.0.2 gives no JavaScript-only meaning
/// load, and keep its answers (`tsc`, which reports TS2339 at `outer.x`
/// and `obj.b`, and none at `S.x`): an assignment to a member of a
/// parameter, of a literal with members, of an existing static, or of a
/// function declared in another container; `this` assignments in a
/// function or at the top level; CommonJS names in an ES module; and JSDoc
/// without a tag the parser reparses at its host.
#[test]
fn constructs_without_javascript_only_meaning_load_with_typescript_answers() {
    let text = r#"export function outer() {}
function g() { outer.x = 1; }
export class S { static x = 1; }
S.x = 2;
export const obj = { a: 1 };
obj.b = 2;
export function ctor() { this.v = 1; }
this.top = 1;
export const e = 1;
exports.q = 2;
module.exports = { e };
export function configure(c) { c.resolve.alias = { a: 1 }; return c; }
/** Plain documentation. */
export const d = 1;
/** @param s the text */
export function u(s) { return s; }
/** @deprecated */
export const old = 1;
/** @returns {number} */
export const notFunction = 1;
export const lit = {
  /**
   * @overload
   * @param {number} a
   */
  m(a) { return a; },
};
"#;
    let use_ts = r#"import { outer, S, obj, d } from "./a.js";
export const t1 = outer.x;
export const t2 = S.x;
export const t3 = obj.b;
export const t4 = d;
"#;
    for checking in [Checking::Plain, Checking::CheckJsFalse] {
        let config = config(checking, r#""src""#, "");
        let fixture = Fixture::new(&[
            ("tsconfig.json", &config),
            ("src/a.js", text),
            ("src/use.ts", use_ts),
        ]);
        let project = fixture
            .try_load()
            .unwrap_or_else(|error| panic!("{checking:?}: {error}"));
        for (line, expected) in [(2, None), (3, Some("number")), (4, None), (5, Some("1"))] {
            match (fixture.type_at(&project, "src/use.ts", line, 14), expected) {
                (Ok(summary), Some(expected)) => assert_eq!(summary, expected, "line {line}"),
                (Err(AnalysisError::Semantic(_)), None) => {}
                (result, expected) => panic!("line {line}: {result:?}, expected {expected:?}"),
            }
        }
    }
}

/// A session's answer after a JavaScript file is edited, after it gains a
/// refused construct, and after it loses it again, each equal to a fresh
/// direct load's.
#[test]
fn a_session_refreshes_javascript_edits_and_refusals_as_a_fresh_load_does() {
    let config = config(Checking::Plain, r#""src""#, "");
    let fixture = Fixture::new(&[
        ("tsconfig.json", &config),
        ("src/lib.js", "export const value = 1;\n"),
        (
            "src/use.ts",
            "import { value } from \"./lib.js\";\nexport const v = value;\n",
        ),
    ]);
    let config = fixture.path("tsconfig.json");
    let request = || {
        ProjectRequest::Type(TypeRequest::new(TypeQuery::At {
            file: fixture.path("src/use.ts"),
            position: SourcePosition {
                line: 2,
                column: 14,
            },
        }))
    };
    let summary = |response: Result<ProjectResponse, AnalysisError>| match response {
        Ok(ProjectResponse::Type(answer)) => match *answer {
            TypeAnswer::Selected(result) => Ok(result.description.summary),
            other => panic!("{other:?}"),
        },
        Ok(other) => panic!("{other:?}"),
        Err(error) => Err(error.to_string()),
    };
    let mut session = ProjectSession::new(&config).unwrap();
    let mut ask = |step: &str| {
        let refreshed = summary(session.execute(request()));
        let fresh = summary(Project::load(&config).and_then(|project| project.execute(request())));
        assert_eq!(refreshed, fresh, "{step}");
        refreshed
    };
    assert_eq!(ask("loaded"), Ok("1".into()));
    std::fs::write(fixture.path("src/lib.js"), "export const value = \"s\";\n").unwrap();
    assert_eq!(ask("edited"), Ok("\"s\"".into()));
    std::fs::write(
        fixture.path("src/lib.js"),
        "export const value = \"s\";\nexport function f() {}\nf.x = 1;\n",
    )
    .unwrap();
    let refused = ask("refused").unwrap_err();
    assert!(
        refused.contains("lib.js:3:1") && refused.contains(&unsupported_javascript(EXPANDO)),
        "{refused}"
    );
    std::fs::write(fixture.path("src/lib.js"), "export const value = 2;\n").unwrap();
    assert_eq!(ask("admitted again"), Ok("2".into()));
}

/// A session whose project changes dialect while its JavaScript stays
/// unchanged answers as a fresh load does: the new dialect selects again
/// among the refusal and decorator candidates captured from the file.
#[test]
fn a_session_reselects_unchanged_javascript_when_the_dialect_changes() {
    let config = config(Checking::Plain, r#""src""#, r#""esModuleInterop": true,"#);
    let fixture = Fixture::new(&[
        ("tsconfig.json", &config),
        ("src/lib.js", "export function Ctor() {\n  this.x = 1;\n}\n"),
        (
            "src/use.ts",
            "import { Ctor } from \"./lib.js\";\nexport const v = Ctor;\n",
        ),
    ]);
    let use_5_6 = |on: bool| {
        if on {
            fixture.claim_compiler(TypeScript56.version());
        } else {
            std::fs::remove_file(fixture.path(MANIFEST)).unwrap();
        }
    };
    let config = fixture.path("tsconfig.json");
    let request = || {
        ProjectRequest::Type(TypeRequest::new(TypeQuery::At {
            file: fixture.path("src/use.ts"),
            position: SourcePosition {
                line: 2,
                column: 14,
            },
        }))
    };
    let summary = |response: Result<ProjectResponse, AnalysisError>| match response {
        Ok(ProjectResponse::Type(answer)) => match *answer {
            TypeAnswer::Selected(result) => Ok(result.description.summary),
            other => panic!("{other:?}"),
        },
        Ok(other) => panic!("{other:?}"),
        Err(error) => Err(error.to_string()),
    };
    let mut session = ProjectSession::new(&config).unwrap();
    let mut ask = |step: &str| {
        let refreshed = summary(session.execute(request()));
        let fresh = summary(Project::load(&config).and_then(|project| project.execute(request())));
        assert_eq!(refreshed, fresh, "{step}");
        refreshed
    };
    let constructor_this = unsupported_javascript(
        "this assignment that TypeScript 5.6 reads as a constructor function member",
    );
    ask("TypeScript 7").unwrap();
    use_5_6(true);
    let refused = ask("TypeScript 5.6").unwrap_err();
    assert!(
        refused.contains("lib.js:2:3") && refused.contains(&constructor_this),
        "{refused}"
    );
    use_5_6(false);
    ask("TypeScript 7 again").unwrap();
    // TypeScript 7 reports an unchecked file's first parameter decorator from
    // its full start, 5.6.3 from its token.
    std::fs::write(
        fixture.path("src/lib.js"),
        "function dec() {}\nexport class Ctor {\n  m(\n    // @ts-ignore\n    @dec\n    @dec x) { return x; }\n}\n",
    )
    .unwrap();
    let decorator = |error: String, at: &str| {
        assert!(
            error.contains(at) && error.contains("Decorators are not valid here."),
            "{error}"
        );
    };
    decorator(ask("decorated, TypeScript 7").unwrap_err(), "lib.js:3:5");
    use_5_6(true);
    decorator(ask("decorated, TypeScript 5.6").unwrap_err(), "lib.js:5:5");
    use_5_6(false);
    decorator(
        ask("decorated, TypeScript 7 again").unwrap_err(),
        "lib.js:3:5",
    );
}

/// Constructs to which TypeScript 5.6.3 alone gives JavaScript-only meaning
/// (its declaration emit shows a constructor function's class, prototype
/// members, a top-level `this` variable, a namespace, an `@enum` type, and
/// an `@constructor` class; 7.0.2's shows none): refused under the
/// TypeScript 5.6 dialect, loaded under TypeScript 7.
const REFUSED_5_6: &[Refused] = &[
    // 5.6.3 binds no `export=` for a name that aliases `exports`
    // (`isExportsOrModuleExportsOrAlias`); 7.0.2 does.
    Refused {
        text: "const e = exports;\nmodule.exports = e;\n",
        at: (2, 1),
        construct: "CommonJS module.exports assignment",
    },
    Refused {
        text: "export function Ctor() {\n  this.x = 1;\n}\n",
        at: (2, 3),
        construct: "this assignment that TypeScript 5.6 reads as a constructor function member",
    },
    Refused {
        text: "export const Expr = function () {\n  this.y = 2;\n};\n",
        at: (2, 3),
        construct: "this assignment that TypeScript 5.6 reads as a constructor function member",
    },
    Refused {
        text: "this.top = 1;\nexport {};\n",
        at: (1, 1),
        construct: "top-level this assignment that TypeScript 5.6 reads as a variable",
    },
    Refused {
        text: "export function F() {}\nF.prototype.m = function () {\n  return 1;\n};\n",
        at: (2, 1),
        construct: "prototype assignment that TypeScript 5.6 reads as a constructor function member",
    },
    Refused {
        text: "export function F() {}\nObject.defineProperty(F.prototype, \"d\", { value: 1 });\n",
        at: (2, 1),
        construct: "prototype assignment that TypeScript 5.6 reads as a constructor function member",
    },
    Refused {
        text: "export const obj = { a: 1 };\nobj.b = 2;\n",
        at: (2, 1),
        construct: "top-level property assignment that TypeScript 5.6 reads as a namespace member",
    },
    Refused {
        text: "export const obj = { a: 1 };\nObject.defineProperty(obj, \"b\", { value: 2 });\n",
        at: (2, 1),
        construct: "top-level Object.defineProperty call that TypeScript 5.6 reads as a namespace member",
    },
    // 5.6.3's `isExpandoSymbol` follows a defaulted initializer and an
    // immediately invoked function, which 7.0.2's `getInitializerSymbol`
    // does not.
    Refused {
        text: "export function make(input) {\n  const f = input || function () {};\n  f.extra = 1;\n  return f;\n}\n",
        at: (3, 3),
        construct: "property assignment that declares a member of a function, class, or object",
    },
    Refused {
        text: "export function iife() {\n  const g = (function () {\n    return function () {};\n  })();\n  g.x = 1;\n  return g;\n}\n",
        at: (5, 3),
        construct: "property assignment that declares a member of a function, class, or object",
    },
    // A variable initialized to `this` stands for it (`isThisInitializedDeclaration`).
    Refused {
        text: "export function S() {\n  const self = this;\n  self.x = 1;\n}\n",
        at: (3, 3),
        construct: "this assignment that TypeScript 5.6 reads as a constructor function member",
    },
    Refused {
        text: "/** @enum {number} */\nexport const E = { A: 1 };\n",
        at: (1, 5),
        construct: "JSDoc @enum tag",
    },
    Refused {
        text: "/** @constructor */\nexport function K() {}\n",
        at: (1, 5),
        construct: "JSDoc @constructor tag",
    },
    // 5.6.3 binds as it walks: before `o` is declared, the assignment
    // declares a global namespace `o` (`jsGlobalAugmentations`).
    Refused {
        text: "o.x = 1;\nexport const o = { a: 1 };\n",
        at: (1, 1),
        construct: "top-level property assignment that TypeScript 5.6 reads as a namespace member",
    },
    // `bindForStatement` binds a loop's body before its incrementor.
    Refused {
        text: "export function run() {\n  for (let i = 0; i < 1; f.x = 1) {\n    var f = undefined || function () {};\n  }\n  return f;\n}\n",
        at: (2, 26),
        construct: "property assignment that declares a member of a function, class, or object",
    },
    // A function in a class member is a `this` container of its own.
    Refused {
        text: "export class C {\n  m() {\n    function inner() {\n      this.z = 1;\n    }\n    return inner;\n  }\n}\n",
        at: (4, 7),
        construct: "this assignment that TypeScript 5.6 reads as a constructor function member",
    },
    // An access statement with a `@type` declares a member
    // (`bindSpecialPropertyDeclaration`), which reads the tag.
    Refused {
        text: "export function F() {}\n/** @type {number} */\nF.x;\n",
        at: (2, 5),
        construct: "JSDoc @type tag",
    },
    Refused {
        text: "export class C {\n  constructor() {\n    /** @type {number} */\n    this.x;\n  }\n}\n",
        at: (3, 9),
        construct: "JSDoc @type tag",
    },
];

fn dialect_fixture(dialect: Dialect, config: &str, text: &str) -> Fixture {
    Fixture::with_dialect(
        dialect,
        &[
            ("tsconfig.json", config),
            ("src/a.js", text),
            ("src/cfg.js", "export const cfg = {};\n"),
        ],
    )
}

#[test]
fn constructs_only_typescript_5_6_gives_meaning_are_refused_in_its_dialect() {
    for refused in REFUSED_5_6 {
        for checking in CHECKING {
            let (text, at) = match checking {
                Checking::NoCheckPragma => (
                    format!("// @ts-nocheck\n{}", refused.text),
                    (refused.at.0 + 1, refused.at.1),
                ),
                _ => (refused.text.to_owned(), refused.at),
            };
            let config = config(checking, r#""src""#, r#""esModuleInterop": true,"#);
            let context = format!("{:?} under {checking:?}", refused.text);
            let fixture = dialect_fixture(TypeScript7, &config, &text);
            fixture
                .try_load()
                .unwrap_or_else(|error| panic!("{context}, TypeScript 7: {error}"));
            let fixture = dialect_fixture(TypeScript56, &config, &text);
            let error = fixture
                .try_load()
                .err()
                .unwrap_or_else(|| panic!("{context}, TypeScript 5.6: loaded"));
            let expected = unsupported_javascript(refused.construct);
            fixture.assert_refused(&error, "src/a.js", at, &expected, &context);
        }
    }
    // 5.6.3 declares nothing for an import's member, a dynamic top-level
    // `this` name, a `void 0` assignment (`getAssignmentDeclarationKind`),
    // or a nested assignment to a literal with members, nor does 7.0.2.
    let config = config(Checking::Plain, r#""src""#, r#""esModuleInterop": true,"#);
    for text in [
        "import { cfg } from \"./cfg.js\";\ncfg.x = 1;\n",
        "const key = \"k\";\nthis[key] = 1;\nexport {};\n",
        "export function F() {\n  this.x = void 0;\n}\n",
        "export function h() {\n  const o = { a: 1 };\n  o.b = 2;\n  return o;\n}\n",
        "import * as ns from \"./cfg.js\";\nns.cfg.x = 1;\n",
        // Nor, binding as it walks, for a variable declared after the
        // assignment.
        "export function swapped() {\n  for (let i = 0; i < 1; ) {\n    f.x = 1;\n    var f = undefined || function () {};\n  }\n  return f;\n}\n",
    ] {
        for dialect in DIALECTS {
            let fixture = dialect_fixture(dialect, &config, text);
            fixture
                .try_load()
                .unwrap_or_else(|error| panic!("{text:?} {dialect:?}: {error}"));
        }
    }
}

const MODULE_EXPORTS_VOID_0: Refused = Refused {
    text: "module.exports = void 0;\n",
    at: (1, 1),
    construct: "CommonJS module.exports assignment",
};

const EXPORTS_VOID_0: Refused = Refused {
    text: "exports.a = void 0;\n",
    at: (1, 1),
    construct: "CommonJS exports assignment",
};

/// An `export=` that both compilers declare: `void 1` is no `void 0`.
const MODULE_EXPORTS_VOID_1: Refused = Refused {
    text: "module.exports = void 1;\n",
    at: (1, 1),
    construct: "CommonJS module.exports assignment",
};

/// Constructs to which TypeScript 7.0.2 alone gives JavaScript-only meaning
/// (its declaration emit shows an `export=`, an export, or a member, and
/// 5.6.3's none; 5.6.3 imports a file whose only CommonJS exports assign
/// `void 0` as a script, TS2306). 5.6.3's `getAssignmentDeclarationKind`
/// declares nothing for an assignment whose rightmost value is `void 0`,
/// while 7.0.2's declares as for any value, and 7.0.2 binds expandos once
/// the container's declarations are all bound, while 5.6.3 binds as it
/// walks. This tool answers the TypeScript 5.6 dialect through the binder
/// and checker that port 7.0.2 for the supported slice alone, which model
/// none of these.
const REFUSED_7: &[Refused] = &[
    MODULE_EXPORTS_VOID_0,
    EXPORTS_VOID_0,
    Refused {
        text: "module.exports.a = void 0;\n",
        at: (1, 1),
        construct: "CommonJS exports assignment",
    },
    Refused {
        text: "exports.a = exports.b = void 0;\n",
        at: (1, 1),
        construct: "CommonJS exports assignment",
    },
    Refused {
        text: "const a = 1;\nmodule.exports = a;\nexports.b = void 0;\n",
        at: (3, 1),
        construct: "CommonJS exports assignment",
    },
    Refused {
        text: "export function f() {}\nf.x = void 0;\n",
        at: (2, 1),
        construct: EXPANDO,
    },
    Refused {
        text: "export const o = {};\no.a = void 0;\n",
        at: (2, 1),
        construct: EXPANDO,
    },
    Refused {
        text: "export class C {\n  constructor() {\n    this.x = void 0;\n  }\n}\n",
        at: (3, 5),
        construct: "this assignment that declares a class or object literal member",
    },
    Refused {
        text: "export function h() {\n  g.x = 1;\n  var g = function () {};\n  return g;\n}\n",
        at: (2, 3),
        construct: EXPANDO,
    },
];

/// Every construct to which only TypeScript 7 gives meaning is refused in
/// both dialects at the same place, whichever route makes its file a
/// project file, as is `module.exports = void 1`, to which both give it.
#[test]
fn constructs_only_typescript_7_gives_meaning_are_refused_in_both_dialects() {
    for refused in REFUSED_7.iter().chain([&MODULE_EXPORTS_VOID_1]) {
        for dialect in DIALECTS {
            for route in [Route::Root, Route::Imported, Route::ReExported] {
                let config = config(
                    Checking::Plain,
                    route.include(),
                    r#""esModuleInterop": true,"#,
                );
                let mut files = vec![
                    ("tsconfig.json", config.as_str()),
                    ("src/lib/a.js", refused.text),
                ];
                files.extend_from_slice(route.files());
                let fixture = Fixture::with_dialect(dialect, &files);
                let context = format!("{:?} {route:?}, {dialect:?}", refused.text);
                let error = fixture
                    .try_load()
                    .err()
                    .unwrap_or_else(|| panic!("{context}: loaded"));
                let expected = unsupported_javascript(refused.construct);
                fixture.assert_refused(&error, "src/lib/a.js", refused.at, &expected, &context);
            }
        }
    }
}

/// In both dialects, a project file's import of a dependency with such a
/// construct fails at the import, and a dependency's own import of it is
/// tolerated, its importer keeping its answers.
#[test]
fn a_dependency_with_a_construct_only_typescript_7_gives_meaning_is_refused_in_both_dialects() {
    let config = config(
        Checking::Plain,
        r#""src""#,
        &format!(r#""esModuleInterop": true, {DEPENDENCY_DEPTH}"#),
    );
    for refused in REFUSED_7.iter().chain([&MODULE_EXPORTS_VOID_1]) {
        for dialect in DIALECTS {
            let context = format!("{:?}, {dialect:?}", refused.text);
            let fixture = Fixture::with_dialect(
                dialect,
                &[
                    ("tsconfig.json", config.as_str()),
                    (
                        "src/use.ts",
                        "import * as ns from \"dep\";\nexport const result = ns;\n",
                    ),
                    DEP_MANIFEST,
                    ("node_modules/dep/index.js", refused.text),
                ],
            );
            let error = fixture
                .try_load()
                .err()
                .unwrap_or_else(|| panic!("{context}: loaded"));
            let (line, column) = refused.at;
            let expected = format!(
                "module \"dep\": node_modules/dep/index.js:{line}:{column}: {}",
                unsupported_javascript(refused.construct)
            );
            fixture.assert_refused(&error, "src/use.ts", (1, 21), &expected, &context);

            let fixture = Fixture::with_dialect(
                dialect,
                &[
                    ("tsconfig.json", config.as_str()),
                    (
                        "src/use.ts",
                        "import { w } from \"wrap\";\nexport const a = w;\n",
                    ),
                    (
                        "node_modules/wrap/package.json",
                        r#"{"name":"wrap","main":"index.js"}"#,
                    ),
                    (
                        "node_modules/wrap/index.js",
                        "import \"dep\";\nexport const w = 1;\n",
                    ),
                    DEP_MANIFEST,
                    ("node_modules/dep/index.js", refused.text),
                ],
            );
            let project = fixture
                .try_load()
                .unwrap_or_else(|error| panic!("{context}: {error}"));
            assert_eq!(
                fixture.summary_at(&project, "src/use.ts", 2, 14),
                "1",
                "{context}"
            );
        }
    }
}

/// A session whose installed compiler changes between TypeScript 7 and 5.6
/// while its CommonJS JavaScript stays unchanged answers as a fresh load
/// does: each dialect refuses the file at the same construct.
#[test]
fn a_session_refuses_unchanged_typescript_7_commonjs_whichever_compiler_is_installed() {
    let config = config(Checking::Plain, r#""src""#, r#""esModuleInterop": true,"#);
    for refused in [MODULE_EXPORTS_VOID_0, EXPORTS_VOID_0, MODULE_EXPORTS_VOID_1] {
        let fixture = Fixture::new(&[
            ("tsconfig.json", &config),
            ("src/lib.js", refused.text),
            (
                "src/use.ts",
                "import * as ns from \"./lib.js\";\nexport const result = ns;\n",
            ),
        ]);
        let config = fixture.path("tsconfig.json");
        let request = || {
            ProjectRequest::Type(TypeRequest::new(TypeQuery::At {
                file: fixture.path("src/use.ts"),
                position: SourcePosition {
                    line: 2,
                    column: 14,
                },
            }))
        };
        let summary = |response: Result<ProjectResponse, AnalysisError>| match response {
            Ok(ProjectResponse::Type(answer)) => match *answer {
                TypeAnswer::Selected(result) => Ok(result.description.summary),
                other => panic!("{other:?}"),
            },
            Ok(other) => panic!("{other:?}"),
            Err(error) => Err(error.to_string()),
        };
        let mut session = ProjectSession::new(&config).unwrap();
        let expected = unsupported_javascript(refused.construct);
        let mut ask = |step: &str| {
            let refreshed = summary(session.execute(request()));
            let fresh =
                summary(Project::load(&config).and_then(|project| project.execute(request())));
            let context = format!("{:?}, {step}", refused.text);
            assert_eq!(refreshed, fresh, "{context}");
            let error = refreshed.expect_err(&context);
            assert!(
                error.contains("lib.js:1:1") && error.contains(&expected),
                "{context}: {error}"
            );
        };
        ask("TypeScript 7");
        install_typescript_5_6(&fixture, true);
        ask("TypeScript 5.6");
        install_typescript_5_6(&fixture, false);
        ask("TypeScript 7 again");
    }
}

/// A session that has published a generation answers as a fresh load does
/// after its installed compiler changes between TypeScript 7 and 5.6 with
/// its JavaScript unchanged. A dependency imports two others: one with
/// CommonJS to which only 7.0.2 gives meaning, refused under both dialects
/// and so naming nothing, and one with a constructor function's `this`
/// assignment, to which only 5.6.3 gives meaning, whose constant answers
/// under TypeScript 7 alone.
#[test]
fn a_session_keeps_typescript_7_commonjs_dependencies_refused_as_the_dialect_changes() {
    let config = config(
        Checking::Plain,
        r#""src""#,
        &format!(r#""esModuleInterop": true, {DEPENDENCY_DEPTH}"#),
    );
    for refused in [MODULE_EXPORTS_VOID_0, EXPORTS_VOID_0, MODULE_EXPORTS_VOID_1] {
        let fixture = Fixture::new(&[
            ("tsconfig.json", &config),
            (
                "src/use.ts",
                "import { w, k } from \"wrap\";\nexport const a = w;\nexport const b = k;\n",
            ),
            (
                "node_modules/wrap/package.json",
                r#"{"name":"wrap","main":"index.js"}"#,
            ),
            (
                "node_modules/wrap/index.js",
                "import * as d from \"dep\";\nexport { k } from \"ctor\";\nexport const w = d;\n",
            ),
            DEP_MANIFEST,
            ("node_modules/dep/index.js", refused.text),
            (
                "node_modules/ctor/package.json",
                r#"{"name":"ctor","main":"index.js"}"#,
            ),
            (
                "node_modules/ctor/index.js",
                "export function Ctor() {\n  this.x = 1;\n}\nexport const k = 1;\n",
            ),
        ]);
        let config = fixture.path("tsconfig.json");
        let request = |line| {
            ProjectRequest::Type(TypeRequest::new(TypeQuery::At {
                file: fixture.path("src/use.ts"),
                position: SourcePosition { line, column: 14 },
            }))
        };
        // A loaded project's answer: a failure is the query's own.
        let answer = |response: Result<ProjectResponse, AnalysisError>| match response {
            Ok(ProjectResponse::Type(answer)) => match *answer {
                TypeAnswer::Selected(result) => Some(result.description.summary),
                other => panic!("{other:?}"),
            },
            Err(AnalysisError::Semantic(_)) => None,
            other => panic!("{other:?}"),
        };
        let mut session = ProjectSession::new(&config).unwrap();
        let mut ask = |step: &str, constant: Option<&str>| {
            let context = format!("{:?}, {step}", refused.text);
            for (line, expected) in [(2, None), (3, constant)] {
                let refreshed = answer(session.execute(request(line)));
                let fresh = answer(
                    Project::load(&config).and_then(|project| project.execute(request(line))),
                );
                assert_eq!(refreshed, fresh, "{context}, line {line}");
                assert_eq!(refreshed.as_deref(), expected, "{context}, line {line}");
            }
        };
        ask("TypeScript 7", Some("1"));
        install_typescript_5_6(&fixture, true);
        ask("TypeScript 5.6", None);
        install_typescript_5_6(&fixture, false);
        ask("TypeScript 7 again", Some("1"));
    }
}

/// Installs, or removes, the compiler that selects the TypeScript 5.6
/// dialect.
fn install_typescript_5_6(fixture: &Fixture, installed: bool) {
    if installed {
        fixture.claim_compiler(TypeScript56.version());
    } else {
        std::fs::remove_file(fixture.path(MANIFEST)).unwrap();
    }
}

/// An ambient module of the same name does not stand in for a refused
/// dependency in a project file's import, which fails at the import as
/// without one, while a dependency's import of it reads the ambient module,
/// as TypeScript's `resolveExternalModule` prefers one.
#[test]
fn an_ambient_module_does_not_bypass_a_refused_dependency_for_a_project_import() {
    let config = config(Checking::Plain, r#""src""#, DEPENDENCY_DEPTH);
    let ambient = (
        "src/ambient.d.ts",
        "declare module \"unsafe\" {\n  export const value: 42;\n}\n",
    );
    let manifest = (
        "node_modules/unsafe/package.json",
        r#"{"name":"unsafe","main":"index.js"}"#,
    );
    let refused = ("node_modules/unsafe/index.js", "module.exports = 1;\n");
    let fixture = Fixture::new(&[
        ("tsconfig.json", &config),
        (
            "src/use.ts",
            "import { value } from \"unsafe\";\nexport const v = value;\n",
        ),
        ambient,
        manifest,
        refused,
    ]);
    let Err(error) = fixture.try_load() else {
        panic!("loaded");
    };
    let expected = format!(
        "module \"unsafe\": node_modules/unsafe/index.js:1:1: {}",
        unsupported_javascript("CommonJS module.exports assignment")
    );
    fixture.assert_refused(&error, "src/use.ts", (1, 23), &expected, "project import");

    let fixture = Fixture::new(&[
        ("tsconfig.json", &config),
        (
            "src/use.ts",
            "import { w } from \"wrap\";\nexport const v = w;\n",
        ),
        (
            "node_modules/wrap/package.json",
            r#"{"name":"wrap","types":"index.d.ts"}"#,
        ),
        (
            "node_modules/wrap/index.d.ts",
            "export { value as w } from \"unsafe\";\n",
        ),
        ambient,
        manifest,
        refused,
    ]);
    let project = fixture.load();
    assert_eq!(fixture.summary_at(&project, "src/use.ts", 2, 14), "42");
}

/// Which JSDoc tags each dialect refuses, by whether its compiler gives the
/// tag meaning where it stands (declaration emit of each): TypeScript 7 by
/// its reparser (the host's last comment; `@overload` in any), 5.6.3 by its
/// ownership (`getJSDocCommentsAndTags`, `filterOwnedJSDocTags`: the last
/// comment of a node a reader's chain reaches, and `@overload` in any) and
/// where its checker reads the tag. The TypeScript 5.6 dialect refuses
/// TypeScript 7's too, which this tool, whose parser ports TypeScript 7's
/// reparser for a variable's `@type` alone, does not model. `None` where the
/// file loads.
#[test]
fn each_dialect_refuses_the_jsdoc_its_compiler_reads() {
    type At = Option<((usize, usize), &'static str)>;
    let cases: [(&str, At, At); 20] = [
        // An earlier comment's `@type`, which neither reads.
        (
            "/** @type {string} */\n/** ordinary documentation */\nexport const value = 42;\n",
            None,
            None,
        ),
        (
            "/** @param {string} a */\nexport const k = 1;\n",
            None,
            None,
        ),
        (
            "/** @returns {number} */\nexport const notFunction = 1;\n",
            None,
            None,
        ),
        // A declaration's `@satisfies` types its initializer contextually:
        // in 5.6.3 a variable's, a parameter's, or a property's
        // (`getContextualTypeForVariableLikeDeclaration`), and in 7.0.2 also
        // an export assignment's or a `return`'s, but not a parameter's,
        // which the TypeScript 5.6 dialect refuses too.
        (
            "/** @satisfies {{ tag: \"x\" | \"y\" }} */\nexport const obj = { tag: \"x\" };\n",
            Some(((1, 5), "JSDoc @satisfies tag")),
            Some(((1, 5), "JSDoc @satisfies tag")),
        ),
        (
            "export const o = {\n  /** @satisfies {{ tag: \"x\" | \"y\" }} */\n  p: { tag: \"x\" },\n};\n",
            Some(((2, 7), "JSDoc @satisfies tag")),
            Some(((2, 7), "JSDoc @satisfies tag")),
        ),
        (
            "export function f(/** @satisfies {{ tag: \"x\" | \"y\" }} */ a = { tag: \"x\" }) {\n  return a;\n}\n",
            None,
            Some(((1, 23), "JSDoc @satisfies tag")),
        ),
        (
            "/** @satisfies {{ tag: \"x\" | \"y\" }} */\nexport default { tag: \"x\" };\n",
            Some(((1, 5), "JSDoc @satisfies tag")),
            Some(((1, 5), "JSDoc @satisfies tag")),
        ),
        (
            "export function g() {\n  /** @satisfies {{ tag: \"x\" | \"y\" }} */\n  return { tag: \"x\" };\n}\n",
            Some(((2, 7), "JSDoc @satisfies tag")),
            Some(((2, 7), "JSDoc @satisfies tag")),
        ),
        // Without an initializer, or in an earlier comment, neither reads it.
        (
            "/** @satisfies {{ tag: \"x\" | \"y\" }} */\nexport let obj;\nobj = { tag: \"x\" };\n",
            None,
            None,
        ),
        (
            "/** @satisfies {{ tag: \"x\" | \"y\" }} */\n/** doc */\nexport const obj = { tag: \"x\" };\n",
            None,
            None,
        ),
        (
            "export const o = {\n  /** @type {string} */\n  a: 1,\n};\n",
            Some(((2, 7), "JSDoc @type tag")),
            Some(((2, 7), "JSDoc @type tag")),
        ),
        (
            "/**\n * @param {string} s\n */\nexport const f = (s) => s;\n",
            Some(((2, 4), "JSDoc @param tag")),
            Some(((2, 4), "JSDoc @param tag")),
        ),
        (
            "export class O {\n  /**\n   * @overload\n   * @param {number} a\n   */\n  /** plain */\n  m(a) {\n    return a;\n  }\n}\n",
            Some(((3, 6), "JSDoc @overload tag")),
            Some(((3, 6), "JSDoc @overload tag")),
        ),
        // 5.6.3 takes `@extends` for any class with an `extends` clause.
        (
            "class Base {}\nclass Other {}\n/** @extends {Other} */\nexport class D extends Base {}\n",
            None,
            Some(((3, 5), "JSDoc @extends tag")),
        ),
        ("/** @this {Date} */\nexport const v = 1;\n", None, None),
        // Neither reads an earlier comment's `@type`, nor `@template` on a
        // constant.
        (
            "/** @type {(a: number) => number} */\n/** doc */\nexport function f(a) {\n  return a;\n}\n",
            None,
            None,
        ),
        ("/** @template T */\nexport const k = 1;\n", None, None),
        // 5.6.3 keeps an object literal member's `@readonly`, but not its
        // accessibility (`getDeclarationModifierFlagsFromSymbol`).
        (
            "export const lit = {\n  /** @readonly */\n  r: 1,\n};\n",
            None,
            Some(((2, 7), "JSDoc @readonly tag")),
        ),
        (
            "export const lit = {\n  /** @private */\n  r() {\n    return 1;\n  },\n};\n",
            None,
            None,
        ),
        // A defaulted assignment's function reads the statement's comment
        // (`getSourceOfDefaultedAssignment`), before the expando member both
        // refuse.
        (
            "export const ns = {};\n/** @param {number} a */\nns.f = ns.f || function (a) {\n  return a;\n};\n",
            Some(((3, 1), EXPANDO)),
            Some(((2, 5), "JSDoc @param tag")),
        ),
    ];
    let config = config(Checking::Plain, r#""src""#, r#""esModuleInterop": true,"#);
    for (text, seven, five) in cases {
        for (dialect, expected) in [(TypeScript7, seven), (TypeScript56, five)] {
            let fixture = dialect_fixture(dialect, &config, text);
            let context = format!("{text:?} under {dialect:?}");
            match expected {
                None => {
                    fixture
                        .try_load()
                        .unwrap_or_else(|error| panic!("{context}: {error}"));
                }
                Some((at, construct)) => {
                    let Err(error) = fixture.try_load() else {
                        panic!("{context}: loaded");
                    };
                    fixture.assert_refused(
                        &error,
                        "src/a.js",
                        at,
                        &unsupported_javascript(construct),
                        &context,
                    );
                }
            }
        }
    }
    // The ignored `@type` leaves the constant's type alone in both.
    for dialect in DIALECTS {
        let fixture = Fixture::with_dialect(
            dialect,
            &[
                ("tsconfig.json", config.as_str()),
                (
                    "src/a.js",
                    "/** @type {string} */\n/** ordinary documentation */\nexport const value = 42;\n",
                ),
                (
                    "src/use.ts",
                    "import { value } from \"./a.js\";\nexport const v = value;\n",
                ),
            ],
        );
        let project = fixture.load();
        assert_eq!(
            fixture.summary_at(&project, "src/use.ts", 2, 14),
            "42",
            "{dialect:?}"
        );
    }
}

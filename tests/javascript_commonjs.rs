//! A CommonJS JavaScript module with one `module.exports = name`, at the
//! final outcome: type and reference queries from a TypeScript importer and
//! in the module. TypeScript 7.0.2's binder makes the file a module
//! (`setCommonJSModuleIndicator`), declares its `module` and `exports`
//! variables (`declareCommonJSVariable`), and binds the assignment as the
//! module's `export=` alias, whose value declaration is the assignment
//! (`bindModuleExportsAssignment`, `JSDeclarationKindModuleExports`). A
//! default import of the module from TypeScript is its synthetic default
//! (`canHaveSyntheticDefault`), which `resolveExternalModuleSymbol` follows
//! through `export=` to the exported name. The fixture is shaped like a
//! web framework's configuration file: a constant annotated with a JSDoc
//! `@type` whose `import("...")` type names a package's interface, with
//! async methods, contextual callbacks, spreads, an empty-array branch,
//! environment reads, and a property assignment to a callback's parameter,
//! none of which declares anything. Expected answers are native 7.0.2's
//! (`tools/type-oracle.mjs`, `tools/semantic-oracle.mjs`) and 5.6.3's
//! (`tools/ts56-oracle.mjs`, declaration emit), which agree on every one.

mod support;

use support::Fixture;
use typescript_parser::project::Project;
use typescript_parser::references::{
    ReferenceAnswer, ReferenceFilters, ReferenceQuery, ReferenceRequest,
};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeAnswer, TypeQuery, TypeRequest};

const CONFIG: &str = r#"{ "compilerOptions": { "allowJs": true, "strict": true, "noEmit": true, "lib": ["es2017"], "types": [], "target": "es2017", "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true }, "include": ["src", "env.d.ts"] }"#;

/// Node's globals, which the configuration reads.
const ENV: &str = "declare var process: { env: { [key: string]: string | undefined } };
declare var __dirname: string;
declare var module: { exports: any };
";

const MANIFEST: &str = r#"{ "name": "framework", "version": "1.0.0", "types": "index.d.ts" }"#;

const INDEX: &str = "export type { AppConfig, Route, Header } from \"./config\";\n";

const DECLARATIONS: &str = "export interface Route { source: string; destination: string; permanent: boolean }
export interface Header { source: string; headers: { key: string; value: string }[]; has?: { type: string; key: string; value?: string }[] }
export type WebpackHook = (config: any, context: { dev: boolean }) => any;
export interface AppConfig {
  transpilePackages?: string[];
  env?: Record<string, string | undefined>;
  poweredByHeader?: boolean;
  generateBuildId?: () => Promise<string | null> | string | null;
  redirects?: () => Promise<Route[]>;
  headers?: () => Promise<Header[]>;
  webpack?: WebpackHook | null;
  turbopack?: { root?: string; resolveAlias?: Record<string, string> };
  experimental?: { proxyTimeout?: number };
}
";

const APP_CONFIG: &str = r#"/**
 * Copies a header when present.
 */
const COPY_HEADER = {
  has: [{ type: "header", key: "Cache-Control" }],
  headers: [{ key: "Cache-Control", value: ":Cache-Control" }],
};

/**
 * @type {import("framework").AppConfig}
 */
const appConfig = {
  /**
   * Prose with a commented-out example:
      webpack: (config) => { return config; },
   */
  poweredByHeader: false,
  env: {
    COMMIT: process.env.COMMIT,
    BUILT: new Date().toISOString(),
    LEVEL: process.env.LEVEL || "error",
  },
  generateBuildId: async () => {
    return process.env.COMMIT || "development";
  },
  async redirects() {
    return [
      ...(process.env.STAGE === "production"
        ? [{ source: "/boom", destination: "/404", permanent: true }]
        : []),
      { source: "/login", destination: "/api/login", permanent: false },
    ];
  },
  async headers() {
    return [
      { source: "/:path*", headers: [{ key: "Cache-Control", value: "no-store" }] },
      { source: "/tiles", ...COPY_HEADER },
    ];
  },
  transpilePackages: ["auth-kit"],
  webpack: (config) => {
    config.resolve.alias = {
      ...config.resolve.alias,
      "map-kit": "./node_modules/map-kit/dist/index.mjs",
    };
    return config;
  },
  turbopack: { root: __dirname, resolveAlias: { "map-kit": "./x.mjs" } },
  experimental: { proxyTimeout: 1000 * 15 * 60 },
};

module.exports = appConfig;
"#;

const SPEC: &str = r#"import appConfig from "../app.config";
export const packages = appConfig.transpilePackages;
export const whole = appConfig;
"#;

fn files<'a>(app_config: &'a str, spec: &'a str) -> Vec<(&'static str, &'a str)> {
    vec![
        ("tsconfig.json", CONFIG),
        ("env.d.ts", ENV),
        ("node_modules/framework/package.json", MANIFEST),
        ("node_modules/framework/index.d.ts", INDEX),
        ("node_modules/framework/config.d.ts", DECLARATIONS),
        ("app.config.js", app_config),
        ("src/config.spec.ts", spec),
    ]
}

/// Both compilers' answers: the default import, its member, the constant
/// and its export, and the contextual callback's parameter.
const ANSWERS: [(&str, usize, usize, &str); 7] = [
    ("src/config.spec.ts", 2, 14, "string[] | undefined"),
    ("src/config.spec.ts", 2, 35, "string[] | undefined"),
    ("src/config.spec.ts", 1, 8, "AppConfig"),
    ("src/config.spec.ts", 3, 14, "AppConfig"),
    ("app.config.js", 12, 7, "AppConfig"),
    ("app.config.js", 52, 18, "AppConfig"),
    ("app.config.js", 41, 13, "any"),
];

fn check_types(fixture: &Fixture, context: &str) {
    let project = fixture.load();
    for (file, line, column, expected) in ANSWERS {
        assert_eq!(
            fixture.summary_at(&project, file, line, column),
            expected,
            "{context}: {file}:{line}:{column}"
        );
    }
}

#[test]
fn a_default_import_of_module_exports_has_the_exported_constants_type() {
    check_types(&Fixture::new(&files(APP_CONFIG, SPEC)), "TypeScript 7");
}

#[test]
fn a_default_import_of_module_exports_has_the_exported_constants_type_under_typescript_5_6() {
    let Some(package) = support::typescript_5_6(
        "a_default_import_of_module_exports_has_the_exported_constants_type_under_typescript_5_6",
    ) else {
        return;
    };
    let fixture = Fixture::new(&files(APP_CONFIG, SPEC));
    fixture.install_compiler(&package);
    check_types(&fixture, "TypeScript 5.6");
}

/// The module is a root as well as an import, and both find it so.
#[test]
fn a_module_exports_root_answers_as_an_imported_one() {
    let config = CONFIG.replace(
        r#""src", "env.d.ts""#,
        r#""src", "env.d.ts", "app.config.js""#,
    );
    let mut all = files(APP_CONFIG, SPEC);
    all[0] = ("tsconfig.json", &config);
    check_types(&Fixture::new(&all), "root");
}

/// Every occurrence of the references at `file:line:column`, sorted.
fn occurrences(
    project: &Project,
    fixture: &Fixture,
    file: &str,
    line: usize,
    column: usize,
) -> Vec<String> {
    let answer = project
        .references(ReferenceRequest {
            query: ReferenceQuery::At {
                file: fixture.path(file),
                position: SourcePosition { line, column },
            },
            filters: ReferenceFilters::default(),
        })
        .unwrap_or_else(|error| panic!("{file}:{line}:{column}: {error}"));
    let ReferenceAnswer::Selected(result) = answer else {
        panic!("{file}:{line}:{column}: {answer:?}");
    };
    assert_eq!(
        format!("{:?}", result.outcome),
        "Complete",
        "{file}:{line}:{column}"
    );
    let mut found: Vec<String> = result
        .groups
        .iter()
        .flat_map(|group| &group.occurrences)
        .map(|occurrence| {
            let start = occurrence.location.start;
            format!(
                "{}:{}:{}",
                occurrence.location.file, start.line, start.column
            )
        })
        .collect();
    found.sort();
    found
}

/// References to the exported constant, from the default import, its
/// uses, the declaration, or the assignment's operand, are the one group
/// both compilers find: the declaration, the operand of `module.exports =`
/// where it was written, and the importer's import and uses.
#[test]
fn references_to_the_exported_constant_reach_the_module_exports_assignment() {
    let fixture = Fixture::new(&files(APP_CONFIG, SPEC));
    let project = fixture.load();
    let mut expected = vec![
        "app.config.js:12:7",
        "app.config.js:52:18",
        "src/config.spec.ts:1:8",
        "src/config.spec.ts:2:25",
        "src/config.spec.ts:3:22",
    ];
    expected.sort();
    for (file, line, column) in [
        ("src/config.spec.ts", 1, 8),
        ("src/config.spec.ts", 2, 25),
        ("app.config.js", 12, 7),
        ("app.config.js", 52, 18),
    ] {
        assert_eq!(
            occurrences(&project, &fixture, file, line, column),
            expected,
            "{file}:{line}:{column}"
        );
    }
}

/// A CommonJS module's own `module` and `exports` variables, which the file
/// declares (`declareCommonJSVariable`) and whose types TypeScript makes of
/// the module (`getTypeOfVariableOrParameterOrPropertyWorker`'s
/// `ModuleExports` branch; native 7.0.2 answers `AppConfig` for
/// `module.exports`), are refused where a query needs their type, while
/// references find the file's declaration and its uses, not a global
/// `module`: both compilers' `module` here is the file's.
#[test]
fn the_commonjs_module_variable_is_the_files_own() {
    let fixture = Fixture::new(&files(APP_CONFIG, SPEC));
    let project = fixture.load();
    assert_eq!(
        occurrences(&project, &fixture, "app.config.js", 52, 1),
        ["app.config.js:52:1"]
    );
    for column in [1, 9] {
        let error = project
            .type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path("app.config.js"),
                position: SourcePosition { line: 52, column },
            }))
            .err()
            .unwrap_or_else(|| panic!("app.config.js:52:{column}: answered"));
        let typescript_parser::analysis::AnalysisError::Semantic(error) = error else {
            panic!("app.config.js:52:{column}: {error:?}");
        };
        assert_eq!(
            error.reason,
            typescript_parser::analysis::SemanticFailureReason::JavascriptSemanticsRequired.into(),
            "{error:?}"
        );
        assert_eq!(
            error.path.as_deref(),
            Some(fixture.path("app.config.js").as_path())
        );
    }
}

/// `import("...")` types of the module from TypeScript
/// (`getTypeFromImportTypeNode`): `typeof` reads the `export=` target's
/// value and its properties, while a type qualifier the target does not
/// export falls back to the module's own exports, the CommonJS branch for
/// typedefs beside `module.exports`, which hold none here: both compilers
/// report TS2694 at the qualifier and give the error type, as without the
/// module a type does (TS1340).
#[test]
fn import_types_of_the_module_follow_its_export_assignment() {
    let types = r#"export declare const v1: typeof import("../app.config");
export declare const v2: typeof import("../app.config").transpilePackages;
export declare const v3: import("../app.config").Missing;
export declare const v4: import("../app.config");
"#;
    let mut all = files(APP_CONFIG, SPEC);
    all.push(("src/types.ts", types));
    let fixture = Fixture::new(&all);
    let project = fixture.load();
    assert_eq!(
        fixture.summary_at(&project, "src/types.ts", 1, 22),
        "AppConfig"
    );
    assert_eq!(
        fixture.summary_at(&project, "src/types.ts", 2, 22),
        "string[] | undefined"
    );
    use typescript_parser::analysis::SemanticFailureReason::{ErrorType, UnresolvedSymbol};
    for (line, at, reason) in [(3, (3, 50), UnresolvedSymbol), (4, (4, 26), ErrorType)] {
        let error = project
            .type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path("src/types.ts"),
                position: SourcePosition { line, column: 22 },
            }))
            .err()
            .unwrap_or_else(|| panic!("src/types.ts:{line}: answered"));
        let typescript_parser::analysis::AnalysisError::Semantic(error) = error else {
            panic!("src/types.ts:{line}: {error:?}");
        };
        assert_eq!(error.reason, reason.into(), "{error:?}");
        let position = error.position.unwrap();
        assert_eq!((position.line, position.column), at, "{error:?}");
    }
}

/// The module's constructs that declare nothing load and answer as native
/// 7.0.2's type oracle does: async methods and arrows typed by their
/// context, spreads of an object and of a conditional with an empty-array
/// branch, environment reads, and a property assignment to a callback's
/// parameter, for which the binder declares no member (`getInitializerSymbol`
/// of a parameter is nil). Each answer is `(file, line, column)`, the range
/// it is for, and the type.
#[test]
fn constructs_that_declare_nothing_answer_as_typescript_7() {
    let fixture = Fixture::new(&files(APP_CONFIG, SPEC));
    let project = fixture.load();
    let object =
        "{ has: { type: string; key: string; }[]; headers: { key: string; value: string; }[]; }";
    let process = "{ env: { [key: string]: string | undefined; }; }";
    for (line, column, expected) in [
        (23, 3, "() => Promise<string>"),
        (
            26,
            9,
            "() => Promise<{ source: string; destination: string; permanent: boolean; }[]>",
        ),
        (
            34,
            9,
            "() => Promise<({ source: string; headers: { key: string; value: string; }[]; } | { has: { type: string; key: string; }[]; headers: { key: string; value: string; }[]; source: string; })[]>",
        ),
        (19, 13, process),
        (19, 25, "string | undefined"),
        (21, 5, "string"),
        (24, 12, process),
        (28, 11, process),
        (37, 31, object),
        (4, 7, object),
        (42, 12, "any"),
        (46, 12, "any"),
        (48, 22, "string"),
    ] {
        assert_eq!(
            fixture.summary_at(&project, "app.config.js", line, column),
            expected,
            "app.config.js:{line}:{column}"
        );
    }
}

/// A session's answers after the module's `@type` is edited, after it
/// switches between `module.exports =` and an ES module default export,
/// and after it gains a CommonJS export this tool refuses and loses it
/// again, each equal to a fresh direct load's.
#[test]
fn a_session_refreshes_commonjs_and_hosted_type_edits_as_a_fresh_load_does() {
    use typescript_parser::analysis::AnalysisError;
    use typescript_parser::project::{ProjectRequest, ProjectResponse, ProjectSession};
    let fixture = Fixture::new(&files(APP_CONFIG, SPEC));
    let config = fixture.path("tsconfig.json");
    let request = |line: usize, column: usize| {
        ProjectRequest::Type(TypeRequest::new(TypeQuery::At {
            file: fixture.path("src/config.spec.ts"),
            position: SourcePosition { line, column },
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
        let mut answers = Vec::new();
        for (line, column) in [(2, 14), (3, 14)] {
            let refreshed = summary(session.execute(request(line, column)));
            let fresh = summary(
                Project::load(&config).and_then(|project| project.execute(request(line, column))),
            );
            assert_eq!(
                refreshed, fresh,
                "{step}: src/config.spec.ts:{line}:{column}"
            );
            answers.push(refreshed);
        }
        answers
    };
    let write = |text: &str| std::fs::write(fixture.path("app.config.js"), text).unwrap();
    let ok = |answers: &[&str]| -> Vec<Result<String, String>> {
        answers
            .iter()
            .map(|answer| Ok((*answer).to_owned()))
            .collect()
    };
    assert_eq!(ask("loaded"), ok(&["string[] | undefined", "AppConfig"]));
    write(&APP_CONFIG.replace(
        "@type {import(\"framework\").AppConfig}",
        "@type {{ transpilePackages: number[] }}",
    ));
    assert_eq!(
        ask("type edited"),
        ok(&["number[]", "{ transpilePackages: number[]; }"])
    );
    write(&APP_CONFIG.replace("module.exports = appConfig;", "export default appConfig;"));
    assert_eq!(ask("ES module"), ok(&["string[] | undefined", "AppConfig"]));
    write(APP_CONFIG);
    assert_eq!(
        ask("CommonJS again"),
        ok(&["string[] | undefined", "AppConfig"])
    );
    write(&format!("{APP_CONFIG}exports.extra = 1;\n"));
    for refused in ask("refused") {
        let refused = refused.unwrap_err();
        assert!(
            refused.contains("app.config.js:53:1")
                && refused.contains("unsupported JavaScript: CommonJS exports assignment"),
            "{refused}"
        );
    }
    write(APP_CONFIG);
    assert_eq!(
        ask("admitted again"),
        ok(&["string[] | undefined", "AppConfig"])
    );
}

/// `canHaveSyntheticDefault` for a `module.exports` module, whose external
/// module indicator is absent or, under `moduleDetection: force`, the file
/// itself: a synthetic default unless the exported value has an
/// `__esModule` member. Both compilers report TS1192 for the marked module
/// and type its import as the error type, and answer `{ a: number; }` for
/// the other, under either detection. The marker is a string-literal
/// member: this tool keys a member declared by an identifier that begins
/// with `__` by TypeScript 6's escaped spelling (docs/references.md), in
/// every file kind, so it does not find one spelled `__esModule:`.
#[test]
fn a_synthetic_default_needs_no_es_module_marker() {
    let use_ts = "import m from \"./marked.js\";\nimport p from \"./plain.js\";\nexport const t1 = m;\nexport const t2 = p;\n";
    for detection in ["auto", "force"] {
        let config = format!(
            r#"{{ "compilerOptions": {{ "allowJs": true, "strict": true, "noEmit": true, "lib": ["es2017"], "types": [], "target": "es2017", "module": "esnext", "moduleResolution": "bundler", "esModuleInterop": true, "moduleDetection": "{detection}" }}, "include": ["src"] }}"#
        );
        let fixture = Fixture::new(&[
            ("tsconfig.json", &config),
            (
                "src/marked.js",
                "/** @type {{ \"__esModule\": true, a: number }} */\nconst c = { \"__esModule\": true, a: 1 };\nmodule.exports = c;\n",
            ),
            (
                "src/plain.js",
                "/** @type {{ a: number }} */\nconst p = { a: 1 };\nmodule.exports = p;\n",
            ),
            ("src/use.ts", use_ts),
        ]);
        let project = fixture.load();
        assert_eq!(
            fixture.summary_at(&project, "src/use.ts", 4, 14),
            "{ a: number; }",
            "{detection}"
        );
        // The import has no default (TS1192 at 1:8): the import's own type
        // and its use's are the error type, each where it is read.
        for (line, column, at) in [(1, 8, (1, 8)), (3, 14, (3, 19))] {
            match project.type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path("src/use.ts"),
                position: SourcePosition { line, column },
            })) {
                Err(typescript_parser::analysis::AnalysisError::Semantic(error)) => {
                    assert_eq!(
                        error.reason,
                        typescript_parser::analysis::SemanticFailureReason::ErrorType.into(),
                        "{detection}: {error:?}"
                    );
                    assert_eq!(
                        error.path.as_deref(),
                        Some(fixture.path("src/use.ts").as_path()),
                        "{detection}"
                    );
                    let position = error.position.unwrap();
                    assert_eq!(
                        (position.line, position.column),
                        at,
                        "{detection}: {error:?}"
                    );
                }
                other => panic!("{detection}: src/use.ts:{line}:{column}: {other:?}"),
            }
        }
    }
}

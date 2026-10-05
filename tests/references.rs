mod support;

use support::Fixture;
use typescript_parser::references::{
    ProjectError, ProjectErrorKind, QueryErrorKind, ReferenceProject, ReferenceQuery,
    SourcePosition,
};

const CONFIG: &str = r#"{ "compilerOptions": { "noLib": true } }"#;
/// TypeScript reports no errors in declaration files under this setting.
const SKIP_LIB_CHECK: &str = r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true } }"#;

fn aliases_and_shadowing() -> Fixture {
    Fixture::new(&[
        ("tsconfig.json", CONFIG),
        (
            "api.ts",
            "export function /*@run:de*/run() {}\nconst /*@copy:d*/copy = /*@run:u*/run;\n/*@copy:u*/copy();\n",
        ),
        (
            "barrel.ts",
            "export { /*@run:e*/run as /*@run:e*/execute } from \"./api\";\n",
        ),
        (
            "app.ts",
            "import { /*@run:i*/execute as /*@run:i*/start } from \"./barrel\";\n/*@run:u*/start();\nfunction /*@local:d*/local(/*@param:d*/start: () => void) { /*@param:u*/start(); }\n",
        ),
    ])
}

#[test]
fn positions_select_the_symbol_behind_aliases_but_not_shadowed_bindings() {
    let fixture = aliases_and_shadowing();
    fixture.check_markers(&fixture.load());
}

#[test]
fn names_select_every_matching_declaration_and_alias() {
    let fixture = aliases_and_shadowing();
    let project = fixture.load();
    fixture.check_name(&project, "start", None, &["run", "param"]);
    fixture.check_name(&project, "start", Some("app.ts"), &["run", "param"]);
    fixture.check_name(&project, "execute", None, &["run"]);
    fixture.check_name(&project, "run", None, &["run"]);
    fixture.check_name(&project, "copy", None, &["copy"]);
    fixture.check_name(&project, "start", Some("api.ts"), &[]);
    fixture.check_name(&project, "missing", None, &[]);
}

fn module_forms() -> Fixture {
    Fixture::new(&[
        ("tsconfig.json", CONFIG),
        (
            "lib.ts",
            r#"export function /*@run:de*/run() {}
export default class /*@Def:de*/Widget {}
export interface /*@Shape:de*/Shape {}
export const /*@ShapeValue:de*/Shape = 1;
const /*@hidden:d*/hidden = 1;
export { /*@hidden:e*/hidden as /*@hidden:e*/visible };
"#,
        ),
        ("anon.ts", "export default function () {}\n"),
        (
            "star.ts",
            r#"export * from "./lib";
export * as /*@lib:e*/lib from "./lib";
export type { /*@run:e*/run as /*@run:e*/typedRun } from "./lib";
"#,
        ),
        (
            "eq.ts",
            r#"function /*@eq:d*/target() {}
namespace /*@eq:d*/target { export const /*@x:de*/x = 1; }
export = /*@eq:e*/target;
"#,
        ),
        (
            "app.ts",
            r#"import /*@Def:i*/Widget2, { /*@run:i*/run, /*@Shape,ShapeValue:i*/Shape, type /*@Shape,ShapeValue:i*/Shape as /*@Shape,ShapeValue:i*/S } from "./lib";
import * as /*@lib:i*/ns from "./lib";
import { /*@run:i*/run as /*@run:i*/again, /*@lib:i*/lib, /*@run:i*/typedRun } from "./star";
import /*@anon:i*/anon from "./anon";
import /*@eq:i*/eq = require("./eq");
new /*@Def:u*/Widget2(); /*@run:u*/run(); /*@run:u*/again();
let /*@s:d*/s: /*@Shape:u*/Shape = /*@ShapeValue:u*/Shape as never;
let /*@t:d*/t: /*@Shape:u*/S = /*@s:u*/s;
/*@?*/S; /*@?*/typedRun();
/*@lib:u*/ns./*@run:u*/run(); /*@lib:u*/lib./*@hidden:u*/visible; /*@lib:u*/ns./*@?*/missing;
/*@anon:u*/anon(); /*@eq:u*/eq./*@x:u*/x; /*@eq:u*/eq(); /*@t:u*/t;
"#,
        ),
    ])
}

#[test]
fn import_and_export_forms_resolve_to_the_underlying_symbol() {
    let fixture = module_forms();
    fixture.check_markers(&fixture.load());
}

#[test]
fn synthetic_declarations_anchor_anonymous_defaults_and_modules() {
    let fixture = module_forms();
    let project = fixture.load();
    let query = |name: &str| {
        project
            .find_references(ReferenceQuery::Name {
                name: name.into(),
                file: Some(fixture.path("app.ts")),
            })
            .unwrap()
    };
    let anon = query("anon");
    assert_eq!(anon.groups.len(), 1);
    let declaration = &anon.groups[0].declarations[0];
    assert_eq!(declaration.name, "default");
    assert_eq!(declaration.location.file, "anon.ts");
    assert_eq!(
        (declaration.location.start, declaration.location.end),
        (at(1, 8), at(1, 15))
    );
    let module = &query("ns").groups[0].declarations;
    assert_eq!(module.len(), 1);
    assert_eq!(module[0].name, "lib.ts");
    assert_eq!(
        (module[0].location.start, module[0].location.end),
        (at(1, 1), at(1, 1))
    );
}

fn at(line: usize, column: usize) -> SourcePosition {
    SourcePosition { line, column }
}

#[test]
fn positions_select_tokens_start_inclusive_end_exclusive() {
    let fixture = aliases_and_shadowing();
    let project = fixture.load();
    let groups = |line, column| {
        project
            .find_references(ReferenceQuery::At {
                file: fixture.path("app.ts"),
                position: at(line, column),
            })
            .map(|result| result.groups.len())
    };
    // `start();` is at line 2 after an 11-column marker comment.
    assert_eq!(groups(2, 11), Ok(1));
    assert_eq!(groups(2, 15), Ok(1));
    assert_eq!(groups(2, 16), Ok(0));
    assert_eq!(groups(2, 10), Ok(0));
}

#[test]
fn invalid_queries_have_typed_errors() {
    let fixture = aliases_and_shadowing();
    std::fs::write(fixture.path("outside.ts"), "let outside = 1;").unwrap();
    let project = fixture.load_config("tsconfig.json");
    let error = |query| project.find_references(query).unwrap_err().kind;
    let position = |file: &str, line, column| ReferenceQuery::At {
        file: fixture.path(file),
        position: at(line, column),
    };
    assert_eq!(
        error(position("app.ts", 0, 1)),
        QueryErrorKind::InvalidPosition
    );
    assert_eq!(
        error(position("app.ts", 99, 1)),
        QueryErrorKind::InvalidPosition
    );
    assert_eq!(
        error(position("app.ts", 1, 999)),
        QueryErrorKind::InvalidPosition
    );
    // Query paths are matched without filesystem access.
    assert_eq!(
        error(position("missing.ts", 1, 1)),
        QueryErrorKind::OutsideProject
    );
    assert_eq!(
        error(ReferenceQuery::Name {
            name: "run".into(),
            file: Some(fixture.path("tsconfig.json")),
        }),
        QueryErrorKind::OutsideProject
    );
}

#[test]
fn columns_inside_surrogate_pairs_are_invalid() {
    let fixture = Fixture::new(&[
        ("tsconfig.json", CONFIG),
        ("astral.ts", "export const 𝒳 = 1;\n"),
    ]);
    let project = fixture.load();
    let query = |column| {
        project.find_references(ReferenceQuery::At {
            file: fixture.path("astral.ts"),
            position: at(1, column),
        })
    };
    assert_eq!(query(14).unwrap().groups.len(), 1);
    assert_eq!(query(15).unwrap_err().kind, QueryErrorKind::InvalidPosition);
    assert_eq!(query(16).unwrap().groups.len(), 0);
}

fn load_error(files: &[(&str, &str)]) -> ProjectError {
    let mut all = vec![("tsconfig.json", CONFIG)];
    all.extend_from_slice(files);
    let fixture = Fixture::new(&all);
    ReferenceProject::load(&fixture.path("tsconfig.json")).unwrap_err()
}

#[test]
fn invalid_projects_report_typed_errors_with_locations() {
    let error = load_error(&[("dup.ts", "export {};\nlet x = 1;\nlet x = 2;\n")]);
    assert_eq!(error.kind, ProjectErrorKind::Binding);
    assert!(error.path.unwrap().ends_with("dup.ts"));
    assert_eq!(error.position, Some(at(3, 5)));

    let error = load_error(&[("missing.ts", "import { x } from \"./nowhere\";\nx;\n")]);
    assert_eq!(error.kind, ProjectErrorKind::UnresolvedImport);
    assert_eq!(error.position, Some(at(1, 19)));

    let error = load_error(&[("with.ts", "declare const o: any;\nwith (o) {}\n")]);
    assert_eq!(error.kind, ProjectErrorKind::Unsupported);
    assert!(error.path.unwrap().ends_with("with.ts"));

    let error = load_error(&[("syntax.ts", "let = ;\n")]);
    assert_eq!(error.kind, ProjectErrorKind::Syntax);
}

#[test]
fn side_effect_imports_must_resolve_unless_unchecked() {
    let error = load_error(&[("app.ts", "import \"./absent\";\nexport const x = 1;\n")]);
    assert_eq!(error.kind, ProjectErrorKind::UnresolvedImport);
    assert_eq!(error.position, Some(at(1, 8)));

    let fixture = project(&[
        ("styles.d.ts", "declare module \"*.css\";\n"),
        (
            "app.ts",
            "import \"./style.css\";\nimport { /*@?*/gone } from \"./lib\";\n/*@?*/gone; /*@?*/console;\n",
        ),
        ("lib.ts", "export const /*@present:de*/present = 1;\n"),
    ]);
    fixture.check_markers(&fixture.load());

    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "noUncheckedSideEffectImports": false } }"#,
        ),
        (
            "app.ts",
            "import \"./absent\";\nexport const /*@x:de*/x = 1;\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn export_equals_namespaces_serve_default_named_and_umd_access() {
    let fixture = Fixture::new(&[
        ("tsconfig.json", CONFIG),
        (
            "react.d.ts",
            r#"export = /*@React:e*/React;
export as namespace /*@React:e*/React;
declare namespace /*@React:d*/React {
  function /*@useState:d*/useState(): void;
  interface /*@Props:d*/Props {}
}
"#,
        ),
        (
            "app.ts",
            r#"import /*@React:i*/React from "./react";
import { /*@useState:i*/useState, type /*@Props:i*/Props } from "./react";
import /*@React:i*/Required = require("./react");
/*@React:u*/React./*@useState:u*/useState(); /*@useState:u*/useState(); /*@React:u*/Required./*@useState:u*/useState();
let /*@p:d*/p: /*@Props:u*/Props | /*@React:u*/React./*@Props:u*/Props;
/*@p:u*/p;
"#,
        ),
        (
            "global.ts",
            "/*@React:u*/React./*@useState:u*/useState();\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn query_paths_match_lexically_after_loading() {
    let fixture = aliases_and_shadowing();
    let project = fixture.load();
    let query = |file: std::path::PathBuf| {
        project
            .find_references(ReferenceQuery::Name {
                name: "start".into(),
                file: Some(file),
            })
            .map(|result| result.groups.len())
    };
    assert_eq!(query(fixture.path("./sub/../app.ts")), Ok(2));
    // The captured index answers even after the file disappears from disk.
    std::fs::remove_file(fixture.path("app.ts")).unwrap();
    assert_eq!(query(fixture.path("app.ts")), Ok(2));
}

#[test]
fn export_assignment_expressions_anchor_at_the_equals_sign() {
    let fixture = Fixture::new(&[
        ("tsconfig.json", CONFIG),
        ("value.ts", "export = 40 + 2;\n"),
        (
            "app.ts",
            "import /*@v:i*/value = require(\"./value\");\n/*@v:u*/value;\n",
        ),
    ]);
    let project = fixture.load();
    fixture.check_markers(&project);
    let result = project
        .find_references(ReferenceQuery::Name {
            name: "value".into(),
            file: None,
        })
        .unwrap();
    let declaration = &result.groups[0].declarations[0];
    assert_eq!(declaration.name, "export=");
    assert_eq!(declaration.location.file, "value.ts");
    assert_eq!(
        (declaration.location.start, declaration.location.end),
        (at(1, 8), at(1, 9))
    );
}

#[test]
fn type_only_restrictions_apply_at_value_uses_not_to_re_exports() {
    let fixture = Fixture::new(&[
        ("tsconfig.json", CONFIG),
        (
            "lib.ts",
            "export interface /*@T:de*/Shape {}\nexport const /*@V:de*/Shape = 1;\n",
        ),
        (
            "types.ts",
            "import type { /*@T,V:i*/Shape } from \"./lib\";\nexport { /*@T,V:e*/Shape };\n",
        ),
        (
            "app.ts",
            "import { /*@T,V:i*/Shape } from \"./types\";\nlet /*@x:d*/x: /*@T:u*/Shape | typeof /*@V:u*/Shape = /*@?*/Shape;\n/*@x:u*/x;\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

fn project(files: &[(&str, &str)]) -> Fixture {
    let mut all = vec![("tsconfig.json", CONFIG)];
    all.extend_from_slice(files);
    Fixture::new(&all)
}

#[test]
fn star_exports_prefer_accessible_routes_to_one_declaration() {
    for barrel in [
        "export type * from \"./c\";\nexport * from \"./c\";\n",
        "export * from \"./c\";\nexport type * from \"./c\";\n",
    ] {
        let fixture = project(&[
            ("c.ts", "export const /*@x:de*/x = 1;\n"),
            ("barrel.ts", barrel),
            (
                "app.ts",
                "import { /*@x:i*/x } from \"./barrel\";\n/*@x:u*/x;\n",
            ),
        ]);
        fixture.check_markers(&fixture.load());
    }
    let fixture = project(&[
        ("c.ts", "export const /*@x:de*/x = 1;\n"),
        ("type.ts", "export type { /*@x:e*/x } from \"./c\";\n"),
        ("value.ts", "export { /*@x:e*/x } from \"./c\";\n"),
        (
            "barrel.ts",
            "export * from \"./type\";\nexport * from \"./value\";\n",
        ),
        (
            "app.ts",
            "import { /*@x:i*/x } from \"./barrel\";\ntype /*@T:d*/T = typeof /*@x:u*/x;\n/*@x:u*/x;\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn export_equals_modules_keep_explicit_exports_and_forward_module_objects() {
    let fixture = project(&[
        (
            "lib.ts",
            "namespace /*@N:d*/N { export const /*@x:de*/x = 1; }\nexport interface /*@T:de*/T {}\nexport = /*@N:e*/N;\n",
        ),
        ("values.ts", "export const /*@v:de*/v = 1;\n"),
        (
            "eq.ts",
            "import * as /*@values:i*/ns from \"./values\";\nexport = /*@values:e*/ns;\n",
        ),
        (
            "app.ts",
            r#"import { /*@T:i*/T, /*@x:i*/x } from "./lib";
import { /*@v:i*/v } from "./eq";
export { /*@T:e*/T as /*@T:e*/Again } from "./lib";
let /*@a:d*/a: /*@T:u*/T = {};
/*@x:u*/x; /*@v:u*/v; /*@a:u*/a;
"#,
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn type_only_re_exports_keep_value_identity_for_typeof() {
    let fixture = project(&[
        (
            "lib.ts",
            "export function /*@f:de*/f() {}\nexport class /*@C:de*/C {}\n",
        ),
        (
            "barrel.ts",
            "import type { /*@f:i*/f, /*@C:i*/C } from \"./lib\";\nexport { /*@f:e*/f, /*@C:e*/C };\n",
        ),
        (
            "app.ts",
            "import type { /*@f:i*/f, /*@C:i*/C } from \"./barrel\";\nlet /*@a:d*/a: typeof /*@f:u*/f | typeof /*@C:u*/C | /*@C:u*/C;\n/*@?*/f(); new /*@?*/C(); /*@a:u*/a;\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn shorthand_ambient_modules_alias_imports_but_declare_no_members() {
    let fixture = project(&[
        ("shorty.d.ts", "declare module \"shorty\";\n"),
        (
            "app.ts",
            "import { /*@m:i*/thing } from \"shorty\";\nimport * as /*@m:i*/ns from \"shorty\";\n/*@m:u*/thing; /*@m:u*/ns./*@?*/other;\nlet /*@t:d*/t: import(\"shorty\")./*@?*/Kind;\n/*@t:u*/t;\nexport {};\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn package_declarations_may_import_missing_peer_modules() {
    let fixture = Fixture::new(&[
        ("tsconfig.json", SKIP_LIB_CHECK),
        (
            "node_modules/tool/package.json",
            r#"{ "name": "tool", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/tool/index.d.ts",
            "import { Options } from \"absent-peer\";\nexport declare function run(options?: Options): void;\nexport { Options };\n",
        ),
        (
            "app.ts",
            "import { /*@run:i*/run, /*@?*/Options } from \"tool\";\n/*@run:u*/run();\nlet /*@o:d*/o: /*@?*/Options | undefined;\n/*@o:u*/o;\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn package_augmentations_of_absent_modules_bind_nothing() {
    let fixture = Fixture::new(&[
        ("tsconfig.json", SKIP_LIB_CHECK),
        (
            "node_modules/plugin/package.json",
            r#"{ "name": "plugin", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/plugin/index.d.ts",
            "export declare const plugin: number;\ndeclare module \"absent-host\" { export interface Options { extra: string } }\n",
        ),
        (
            "app.ts",
            "import { /*@plugin:i*/plugin } from \"plugin\";\n/*@plugin:u*/plugin;\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn suppressed_unresolved_imports_leave_names_unresolved() {
    let fixture = project(&[(
        "app.ts",
        "// @ts-expect-error The package does not export this path.\nexport type /*@T:de*/T = import(\"pkg/hidden\")./*@?*/Brand;\n  /* @ts-ignore */\nimport { /*@?*/gone } from \"./absent\";\n/*@?*/gone;\n",
    )]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn generated_declarations_may_nest_deeper_than_the_default_parse_budget() {
    let deep = |levels: usize| {
        format!(
            "export declare const /*@value:de*/value: {}string{};\n",
            "z.ZodOptional<z.ZodArray<".repeat(levels),
            ", \"many\">>".repeat(levels)
        )
    };
    let fixture = project(&[
        ("generated.d.ts", &deep(30)),
        (
            "z.d.ts",
            "declare namespace z { type ZodOptional<T> = T; type ZodArray<T, M> = T[]; }\n",
        ),
        (
            "app.ts",
            "import { /*@value:i*/value } from \"./generated\";\n/*@value:u*/value;\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());

    let error = load_error(&[("hostile.ts", &deep(5_000))]);
    assert_eq!(error.kind, ProjectErrorKind::Syntax);
}

#[test]
fn declaration_modules_without_a_default_export_have_a_synthetic_default() {
    let fixture = project(&[
        (
            "os.d.ts",
            "export declare function /*@totalmem:de*/totalmem(): number;\n",
        ),
        (
            "ambient.d.ts",
            "declare module \"node:path\" { export function /*@join:de*/join(): string; }\n",
        ),
        ("esm.ts", "export function /*@run:de*/run() {}\n"),
        (
            "app.ts",
            "import /*@os:i*/os from \"./os\";\nimport /*@path:i*/path from \"node:path\";\nimport /*@?*/esm from \"./esm\";\n/*@os:u*/os./*@totalmem:u*/totalmem(); /*@path:u*/path./*@join:u*/join(); /*@?*/esm./*@!*/run;\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn members_beside_a_value_are_properties_and_type_exports_do_not_instantiate() {
    let fixture = project(&[
        (
            "path.d.ts",
            "declare namespace /*@ns:d*/path { interface /*@P:d*/PlatformPath { /*@!*/resolve(): string } }\ndeclare const /*@path:d*/path: /*@ns:u*/path./*@P:u*/PlatformPath;\nexport = /*@ns,path:e*/path;\n",
        ),
        (
            "globals.d.ts",
            "interface /*@E:d*/EventMap {}\ndeclare var /*@proc:d*/proc: number;\ndeclare namespace /*@procns:d*/proc { export { /*@E:e*/EventMap }; }\n",
        ),
        (
            "app.ts",
            "import { /*@!*/resolve } from \"./path\";\n/*@!*/resolve; /*@proc:u*/proc;\nlet /*@m:d*/m: /*@procns:u*/proc./*@E:u*/EventMap;\n/*@m:u*/m;\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn an_es_module_marker_prevents_a_synthetic_default() {
    let fixture = project(&[
        (
            "lib.d.ts",
            "export declare const /*@marker:de*/__esModule: true;\nexport declare const /*@x:de*/x: number;\n",
        ),
        (
            "ambient.d.ts",
            "declare module \"m\" { export const /*@m:de*/__esModule: true; export const /*@y:de*/y: number; }\n",
        ),
        (
            "app.ts",
            "import /*@?*/D from \"./lib\";\nimport /*@?*/M from \"m\";\n/*@?*/D./*@!*/x; /*@?*/M./*@!*/y;\nexport {};\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn failed_explicit_default_exports_stay_unresolved() {
    let fixture = project(&[
        ("lib.d.ts", "export { /*@?*/missing as /*@?*/default };\n"),
        (
            "app.ts",
            "import /*@?*/D from \"./lib\";\n/*@?*/D./*@!*/x;\nexport {};\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn automatic_jsx_runtimes_enter_the_program() {
    let runtime = |global: &str| {
        format!(
            "declare global {{ interface {global} {{}} }}\nexport namespace JSX {{ interface IntrinsicElements {{ div: {{}} }} interface Element {{}} }}\n"
        )
    };
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "jsx": "react-jsx" } }"#,
        ),
        ("node_modules/react/jsx-runtime.d.ts", &runtime("FromReact")),
        (
            "node_modules/preact/jsx-runtime.d.ts",
            &runtime("FromPreact"),
        ),
        ("app.tsx", "export const /*@e:de*/e = <div />;\n"),
        ("plain.tsx", "export const /*@p:de*/p = 1;\n"),
        (
            "pragma.tsx",
            "/** @jsxImportSource preact */\nexport const /*@q:de*/q = <div />;\n",
        ),
        (
            "uses.ts",
            "let /*@a:d*/a: FromReact | FromPreact;\n/*@a:u*/a;\n",
        ),
    ]);
    let project = fixture.load();
    fixture.check_markers(&project);
    for name in ["FromReact", "FromPreact"] {
        let result = project
            .find_references(ReferenceQuery::Name {
                name: name.into(),
                file: None,
            })
            .unwrap();
        assert!(
            result.groups.is_empty(),
            "package declarations are not candidates"
        );
    }
    let position = |line, column| ReferenceQuery::At {
        file: fixture.path("uses.ts"),
        position: at(line, column),
    };
    let declaration =
        |query| project.find_references(query).unwrap().groups[0].declarations[0].clone();
    assert_eq!(
        declaration(position(1, 16)).location.file,
        "node_modules/react/jsx-runtime.d.ts"
    );
    assert_eq!(
        declaration(position(1, 28)).location.file,
        "node_modules/preact/jsx-runtime.d.ts"
    );

    let missing = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "jsx": "react-jsx" } }"#,
        ),
        ("plain.tsx", "export const p = 1;\n"),
    ]);
    missing.load();
    std::fs::write(missing.path("view.tsx"), "export const v = <div />;\n").unwrap();
    let error = ReferenceProject::load(&missing.path("tsconfig.json")).unwrap_err();
    assert_eq!(error.kind, ProjectErrorKind::UnresolvedImport);
}

#[cfg(unix)]
#[test]
fn files_selected_through_symlinks_or_other_cases_can_be_queried() {
    let fixture = project(&[("app.ts", "export const x = 1;\nx;\n")]);
    std::os::unix::fs::symlink(fixture.path("app.ts"), fixture.path("alias.ts")).unwrap();
    std::fs::write(
        fixture.path("tsconfig.json"),
        r#"{ "compilerOptions": { "noLib": true }, "files": ["alias.ts"] }"#,
    )
    .unwrap();
    let project = fixture.load();
    let groups = |file: &str| {
        project
            .find_references(ReferenceQuery::At {
                file: fixture.path(file),
                position: at(2, 1),
            })
            .map(|result| result.groups.len())
    };
    assert_eq!(groups("alias.ts"), Ok(1));
    assert_eq!(groups("app.ts"), Ok(1));
    // On a case-insensitive file system, other spellings name the same file.
    if fixture.path("APP.TS").exists() {
        assert_eq!(groups("APP.TS"), Ok(1));
        std::fs::write(
            fixture.path("tsconfig.json"),
            r#"{ "compilerOptions": { "noLib": true }, "files": ["APP.TS"] }"#,
        )
        .unwrap();
        let project = fixture.load();
        let result = project.find_references(ReferenceQuery::Name {
            name: "x".into(),
            file: None,
        });
        assert_eq!(result.unwrap().groups.len(), 1);
    }
}

#[test]
fn reference_directives_in_selected_package_files_must_resolve() {
    // An explicitly selected root is searched even inside `node_modules`.
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["app.ts", "node_modules/explicit.d.ts"] }"#,
        ),
        ("app.ts", "export const x = 1;\n"),
        (
            "node_modules/explicit.d.ts",
            "/// <reference path=\"./absent.d.ts\" />\ninterface G {}\n",
        ),
    ]);
    let error = ReferenceProject::load(&fixture.path("tsconfig.json")).unwrap_err();
    assert_eq!(error.kind, ProjectErrorKind::UnresolvedImport);
}

#[test]
fn long_lines_convert_positions_in_linear_time() {
    // `é` is one UTF-16 unit and two UTF-8 bytes.
    let source = format!("export const x = \"é\";{}\n", "x;".repeat(40_000));
    let fixture = project(&[("app.ts", &source)]);
    let config = fixture.path("tsconfig.json");
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let project = ReferenceProject::load(&config).unwrap();
        let query = ReferenceQuery::Name {
            name: "x".into(),
            file: None,
        };
        let mut groups = project.find_references(query).unwrap().groups;
        let occurrences = groups.remove(0).occurrences;
        let last = occurrences.last().unwrap().location.clone();
        send.send((occurrences.len(), last.start, last.end))
            .unwrap();
    });
    let (count, start, end) = receive
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("the query should finish");
    assert_eq!(count, 40_001);
    assert_eq!((start, end), (at(1, 80_020), at(1, 80_021)));
}

#[test]
fn many_suppressed_imports_load_in_linear_time() {
    let source: String = (0..8_000)
        .map(|i| format!("// @ts-ignore\nimport {{ x as x{i} }} from \"./absent\";\n"))
        .chain(["export const y = 1;\n".to_owned()])
        .collect();
    let fixture = project(&[("app.ts", &source)]);
    let config = fixture.path("tsconfig.json");
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        send.send(ReferenceProject::load(&config).is_ok()).unwrap();
    });
    let loaded = receive
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("loading should finish");
    assert!(loaded);
}

#[test]
fn block_comment_directives_count_on_their_last_line() {
    let import = "import { x } from \"./missing\";\nexport const y = 1;\n";
    for directive in [
        "/* explanation\n * @ts-ignore */",
        "/* explanation\n @ts-expect-error */",
        "//// @ts-ignore",
    ] {
        project(&[("app.ts", &format!("{directive}\n{import}"))]).load();
    }
    for directive in ["/* @ts-ignore\n explanation */", "/*\n * @ts-ignore\n */"] {
        let error = load_error(&[("app.ts", &format!("{directive}\n{import}"))]);
        assert_eq!(
            error.kind,
            ProjectErrorKind::UnresolvedImport,
            "{directive}"
        );
    }
}

#[test]
fn comment_directives_suppress_missing_reference_directives() {
    for directive in [
        "/// <reference path=\"./missing.ts\" />",
        "/// <reference types=\"absent\" />",
    ] {
        let suppressed = format!("// @ts-ignore\n{directive}\nexport const y = 1;\n");
        project(&[("app.ts", &suppressed)]).load();
        let error = load_error(&[("app.ts", &format!("{directive}\nexport const y = 1;\n"))]);
        assert_eq!(
            error.kind,
            ProjectErrorKind::UnresolvedImport,
            "{directive}"
        );
    }
}

#[test]
fn skip_lib_check_tolerates_unresolved_inputs_only_in_declaration_files() {
    let manifest = |types: &str| format!(r#"{{ "types": "{types}" }}"#);
    let packages = [
        (
            "types",
            "index.d.ts",
            "/// <reference types=\"absent-types\" />\nexport declare const x: number;\n",
        ),
        (
            "path",
            "index.d.ts",
            "/// <reference path=\"./missing.d.ts\" />\nexport declare const x: number;\n",
        ),
        (
            "import",
            "index.d.ts",
            "import { Gone } from \"absent-peer\";\nexport declare const x: Gone;\n",
        ),
        (
            "source",
            "index.ts",
            "import { Gone } from \"absent-peer\";\nexport const x = 1;\n",
        ),
    ];
    for skip in [false, true] {
        let config = format!(
            r#"{{ "compilerOptions": {{ "noLib": true, "module": "nodenext", "skipLibCheck": {skip} }}, "files": ["app.ts"] }}"#
        );
        for (name, file, text) in packages {
            for local in [false, true] {
                let (import, files) = if local {
                    (
                        format!("./lib/{file}")
                            .replace(".d.ts", "")
                            .replace(".ts", ""),
                        vec![(format!("lib/{file}"), text.to_owned())],
                    )
                } else {
                    (
                        name.to_owned(),
                        vec![
                            (format!("node_modules/{name}/package.json"), manifest(file)),
                            (format!("node_modules/{name}/{file}"), text.to_owned()),
                        ],
                    )
                };
                let app = format!("import {{ x }} from \"{import}\";\nexport const y = x;\n");
                let mut all = vec![("tsconfig.json", config.as_str()), ("app.ts", app.as_str())];
                all.extend(
                    files
                        .iter()
                        .map(|(path, text)| (path.as_str(), text.as_str())),
                );
                let fixture = Fixture::new(&all);
                let result = ReferenceProject::load(&fixture.path("tsconfig.json"));
                let tolerated = skip && file.ends_with(".d.ts");
                match result {
                    Ok(_) => assert!(tolerated, "{name} {file} local={local} skip={skip} loaded"),
                    Err(error) => {
                        assert!(
                            !tolerated,
                            "{name} {file} local={local} skip={skip}: {error}"
                        );
                        assert_eq!(error.kind, ProjectErrorKind::UnresolvedImport, "{error}");
                    }
                }
            }
        }
    }
}

#[test]
fn missing_jsx_runtimes_are_reported_at_the_first_jsx_tag() {
    let config = |checked: bool| {
        format!(
            r#"{{ "compilerOptions": {{ "noLib": true, "jsx": "react-jsx", "noUncheckedSideEffectImports": {checked} }} }}"#
        )
    };
    let load = |checked: bool, source: &str| {
        let fixture = Fixture::new(&[("tsconfig.json", &config(checked)), ("app.tsx", source)]);
        ReferenceProject::load(&fixture.path("tsconfig.json")).map(|_| ())
    };
    // Unchecked side-effect imports do not include the runtime.
    let error = load(false, "export const y = <div />;\n").unwrap_err();
    assert_eq!(error.kind, ProjectErrorKind::UnresolvedImport);
    assert_eq!(error.position, Some(at(1, 18)));
    // A directive before the first tag suppresses the file's one error.
    load(
        true,
        "export {};\n// @ts-ignore\nconst y = <div />;\nconst z = <span />;\n",
    )
    .unwrap();
    let error = load(
        true,
        "export {};\nconst y = <div />;\n// @ts-ignore\nconst z = <span />;\n",
    )
    .unwrap_err();
    assert_eq!(error.position, Some(at(2, 11)));
}

#[test]
fn jsx_pragmas_are_read_from_block_comments() {
    let load = |source: &str| {
        let fixture = Fixture::new(&[
            (
                "tsconfig.json",
                r#"{ "compilerOptions": { "noLib": true, "jsx": "react-jsx" } }"#,
            ),
            (
                "node_modules/react/jsx-runtime.d.ts",
                "export namespace JSX { interface IntrinsicElements { div: {} } interface Element {} }\n",
            ),
            ("app.tsx", source),
        ]);
        ReferenceProject::load(&fixture.path("tsconfig.json")).map(|_| ())
    };
    load("// @jsxImportSource preact\nexport const e = <div />;\n").unwrap();
    for pragma in [
        "/* @JSXIMPORTSOURCE preact */",
        "/**\n * @jsxImportSource preact\n */",
    ] {
        let error = load(&format!("{pragma}\nexport const e = <div />;\n")).unwrap_err();
        assert_eq!(error.kind, ProjectErrorKind::UnresolvedImport, "{pragma}");
    }
}

#[test]
fn suppression_directives_are_real_comments_on_preceding_lines() {
    // A directive inside a block comment is not a directive.
    let error = load_error(&[(
        "app.ts",
        "/*\n// @ts-ignore\n*/ import { x } from \"./missing\";\nexport const y = 1;\n",
    )]);
    assert_eq!(error.kind, ProjectErrorKind::UnresolvedImport);
    // A directive applies across blank and comment-only lines.
    let fixture = project(&[(
        "app.ts",
        "// @ts-ignore\n\n// explanation\nimport { /*@?*/x } from \"./missing\";\nexport const /*@y:de*/y = 1;\n// @ts-expect-error\ndeclare module \"./also-missing\" { }\n",
    )]);
    fixture.check_markers(&fixture.load());
    // Code between the directive and the import ends its reach.
    let error = load_error(&[(
        "app.ts",
        "// @ts-ignore\nexport const z = 1;\nimport { x } from \"./missing\";\n",
    )]);
    assert_eq!(error.kind, ProjectErrorKind::UnresolvedImport);
}

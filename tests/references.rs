mod support;

use support::{CONFIG, Fixture, QueryFailure, load_error, project};
use typescript_parser::analysis::{InputErrorKind, QueryErrorKind};
use typescript_parser::project::Project;
use typescript_parser::references::ReferenceQuery;
use typescript_parser::source::SourcePosition;

/// TypeScript reports no errors in declaration files under this setting.
const SKIP_LIB_CHECK: &str = r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true } }"#;

/// Cross-file aliases and a shadowing parameter; tests/reference_fixtures.rs
/// checks its position queries.
const ALIASES: &str = "aliases-shadowing";

#[test]
fn names_select_every_matching_declaration_and_alias() {
    let fixture = Fixture::checked_in(ALIASES);
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
    let fixture = Fixture::checked_in(ALIASES);
    let project = fixture.load();
    let groups = |line, column| {
        project
            .find_references(ReferenceQuery::At {
                file: fixture.path("app.ts"),
                position: at(line, column),
            })
            .map(|result| result.groups.len())
    };
    // `start();` is at line 3 after a 10-column marker comment.
    assert_eq!(groups(3, 11), Ok(1));
    assert_eq!(groups(3, 15), Ok(1));
    assert_eq!(groups(3, 16), Ok(0));
    assert_eq!(groups(3, 10), Ok(0));
}

#[test]
fn invalid_queries_have_typed_errors() {
    let fixture = Fixture::checked_in(ALIASES);
    let project = fixture.load();
    let error = |query| project.find_references(query).query_error().kind;
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
    assert_eq!(
        query(15).query_error().kind,
        QueryErrorKind::InvalidPosition
    );
    assert_eq!(query(16).unwrap().groups.len(), 0);
}

#[test]
fn invalid_projects_report_typed_errors_with_locations() {
    let error = load_error(&[("dup.ts", "export {};\nlet x = 1;\nlet x = 2;\n")]);
    assert_eq!(error.kind, InputErrorKind::Binding);
    assert!(error.path.unwrap().ends_with("dup.ts"));
    assert_eq!(error.position, Some(at(3, 5)));

    let error = load_error(&[("missing.ts", "import { x } from \"./nowhere\";\nx;\n")]);
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport);
    assert_eq!(error.position, Some(at(1, 19)));

    let error = load_error(&[("with.ts", "declare const o: any;\nwith (o) {}\n")]);
    assert_eq!(error.kind, InputErrorKind::Unsupported);
    assert!(error.path.unwrap().ends_with("with.ts"));

    let error = load_error(&[("syntax.ts", "let = ;\n")]);
    assert_eq!(error.kind, InputErrorKind::Syntax);
}

#[test]
fn side_effect_imports_must_resolve_unless_unchecked() {
    let error = load_error(&[("app.ts", "import \"./absent\";\nexport const x = 1;\n")]);
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport);
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
    // A copy, because the test deletes a file.
    let fixture = Fixture::copy_of(ALIASES);
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
#[cfg(unix)]
fn query_paths_clamp_parent_traversal_at_the_root() {
    let fixture = Fixture::checked_in(ALIASES);
    let project = fixture.load();
    let file = std::path::Path::new("/../").join(fixture.path("app.ts").strip_prefix("/").unwrap());
    let result = project
        .find_references(ReferenceQuery::Name {
            name: "start".into(),
            file: Some(file),
        })
        .unwrap();
    assert_eq!(result.groups.len(), 2);
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
fn alias_cycles_resolve_through_local_declarations_whichever_file_loads_first() {
    // TypeScript resolves a name with local declarations to those locals
    // without following a same-named import, so an import that comes back
    // through `b` reaches `a`'s own `const Foo`. That conflict is TS2440, a
    // binding error in a checked file; a dependency tolerates it.
    let a = "import { Foo } from \"./b\";\nconst Foo = 1;\nexport { Foo };\n";
    let b = "export { Foo } from \"./a\";\n";
    for order in [r#"["a.ts", "b.ts"]"#, r#"["b.ts", "a.ts"]"#] {
        let config = format!(r#"{{ "compilerOptions": {{ "noLib": true }}, "files": {order} }}"#);
        let fixture = Fixture::new(&[("tsconfig.json", &config), ("a.ts", a), ("b.ts", b)]);
        let error = Project::load(&fixture.path("tsconfig.json"))
            .map_err(support::input_error)
            .unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Binding, "{order}: {error}");
        assert!(error.path.unwrap().ends_with("a.ts"));
        assert_eq!(error.position, Some(at(1, 10)), "{order}");
    }
    let package = [
        (
            "node_modules/pkg/package.json",
            r#"{ "name": "pkg", "types": "a.d.ts" }"#,
        ),
        (
            "node_modules/pkg/a.d.ts",
            "import { Foo } from \"./b\";\ndeclare const Foo: number;\nexport { Foo };\n",
        ),
        ("node_modules/pkg/b.d.ts", b),
        (
            "via-a.ts",
            "import { /*@foo:i*/Foo } from \"pkg/a\";\n/*@foo:u*/Foo;\n",
        ),
        (
            "via-b.ts",
            "import { /*@foo:i*/Foo } from \"pkg/b\";\n/*@foo:u*/Foo;\n",
        ),
    ];
    for order in [r#"["via-a.ts", "via-b.ts"]"#, r#"["via-b.ts", "via-a.ts"]"#] {
        let config = format!(
            r#"{{ "compilerOptions": {{ "noLib": true, "skipLibCheck": true }}, "files": {order} }}"#
        );
        let mut files = vec![("tsconfig.json", config.as_str())];
        files.extend_from_slice(&package);
        let fixture = Fixture::new(&files);
        let project = fixture.load();
        fixture.check_markers(&project);
        let result = project
            .find_references(ReferenceQuery::Name {
                name: "Foo".into(),
                file: Some(fixture.path("via-b.ts")),
            })
            .unwrap();
        assert_eq!(
            result.groups[0].declarations[0].location.file, "node_modules/pkg/a.d.ts",
            "{order}"
        );
    }
}

#[test]
fn exporting_a_merged_local_declaration_is_not_a_conflict() {
    // tsc 7.0.2 accepts both; an export specifier of a local names the local
    // itself.
    for source in [
        "export interface B { a: number }\nconst B = 1;\nexport { B };\n",
        "export type T = number;\nnamespace T { export const q = 1; }\nexport { T };\n",
    ] {
        project(&[("a.ts", source)]).load();
    }
    // An import that resolves back to its own module's local is TS2440.
    let error = load_error(&[("a.ts", "import { x } from \"./a\";\nexport const x = 1;\n")]);
    assert_eq!(error.kind, InputErrorKind::Binding, "{error}");
    assert_eq!(error.position, Some(at(1, 10)));
}

#[test]
fn alias_conflicts_use_the_alias_location_and_suppression_in_either_order() {
    for (statement, column) in [
        ("import { x } from \"./lib\";", 10),
        ("import { original as x } from \"./lib\";", 10),
        ("import x from \"./lib\";", 8),
        ("import * as x from \"./lib\";", 13),
        ("import x = require(\"./eq\");", 1),
    ] {
        for alias_first in [true, false] {
            for suppressed in ["none", "alias", "local"] {
                let alias = format!(
                    "{}{statement}",
                    if suppressed == "alias" {
                        "// @ts-ignore\n"
                    } else {
                        ""
                    }
                );
                let local = format!(
                    "{}const x = 1;",
                    if suppressed == "local" {
                        "// @ts-ignore\n"
                    } else {
                        ""
                    }
                );
                let source = if alias_first {
                    format!("{alias}\n{local}\n")
                } else {
                    format!("{local}\n{alias}\n")
                };
                let line = source[..source.find(statement).unwrap()]
                    .bytes()
                    .filter(|&byte| byte == b'\n')
                    .count()
                    + 1;
                let fixture = project(&[
                    (
                        "lib.ts",
                        "export const x = 1; export { x as original }; export default x;\n",
                    ),
                    (
                        "eq.ts",
                        "declare namespace target { const q: number; } export = target;\n",
                    ),
                    ("app.ts", &source),
                ]);
                let result =
                    Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error);
                if suppressed == "alias" {
                    assert!(result.is_ok(), "{source}: {result:?}");
                } else {
                    let error = result.unwrap_err();
                    assert_eq!(error.kind, InputErrorKind::Binding, "{source}: {error}");
                    assert_eq!(error.position, Some(at(line, column)), "{source}");
                    assert!(error.path.unwrap().ends_with("app.ts"));
                }
            }
        }
    }
    let error = load_error(&[
        ("lib.ts", "export const x = 1;\n"),
        (
            "app.ts",
            "import { x as a, x as b } from \"./lib\";\nconst b = 1;\nconst a = 1;\n",
        ),
    ]);
    assert_eq!(error.kind, InputErrorKind::Binding);
    assert_eq!(error.position, Some(at(1, 10)));
}

#[test]
fn cut_alias_cycles_do_not_leak_into_other_lookups() {
    // `m` and `n` each declare one meaning of `x` and re-export the other's,
    // a cycle of fill edges. Whichever lookup reaches it first, `c`'s value
    // use resolves to `n`'s constant.
    let sources = [
        (
            "node_modules/m/index.d.ts",
            "export interface x {}\nexport { x } from \"n\";\n",
        ),
        (
            "node_modules/n/index.d.ts",
            "export declare const x: number;\nexport { x } from \"m\";\n",
        ),
        ("c.ts", "import { x } from \"m\";\nx;\n"),
        ("b.ts", "export { x } from \"n\";\n"),
    ];
    for order in [r#"["c.ts", "b.ts"]"#, r#"["b.ts", "c.ts"]"#] {
        let config = format!(
            r#"{{ "compilerOptions": {{ "noLib": true, "skipLibCheck": true }}, "files": {order} }}"#
        );
        let mut files = vec![("tsconfig.json", config.as_str())];
        files.extend_from_slice(&sources);
        let fixture = Fixture::new(&files);
        let result = fixture
            .load()
            .find_references(ReferenceQuery::At {
                file: fixture.path("c.ts"),
                position: at(2, 1),
            })
            .unwrap_or_else(|error| panic!("{order}: {error}"));
        let declarations: Vec<_> = result.groups[0]
            .declarations
            .iter()
            .map(|declaration| declaration.location.file.as_str())
            .collect();
        assert_eq!(declarations, ["node_modules/n/index.d.ts"], "{order}");
    }
}

#[test]
fn alias_cycles_without_declarations_are_unresolved() {
    // TypeScript's TS2303: every alias in the cycle is unresolved, whichever
    // file resolution reaches first.
    let sources = [
        ("d.ts", "export { /*@?*/Bar } from \"./e\";\n"),
        ("e.ts", "export { /*@?*/Bar } from \"./d\";\n"),
        ("f.ts", "import { /*@?*/Bar } from \"./e\";\n/*@?*/Bar;\n"),
    ];
    for order in [r#"["d.ts", "e.ts", "f.ts"]"#, r#"["f.ts", "e.ts", "d.ts"]"#] {
        let config = format!(r#"{{ "compilerOptions": {{ "noLib": true }}, "files": {order} }}"#);
        let mut files = vec![("tsconfig.json", config.as_str())];
        files.extend_from_slice(&sources);
        let fixture = Fixture::new(&files);
        fixture.check_markers(&fixture.load());
    }
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
fn mixed_export_declarations_are_binding_errors_only_in_checked_files() {
    // Declarations of one name in one container must be all exported or all
    // local (TypeScript's TS2395 and TS2383). `export {}` stops an ambient
    // module from exporting every declaration.
    let mixed =
        "export declare function f(): void;\ndeclare function f(x: number): void;\nexport {};\n";
    let error = load_error(&[("mixed.ts", mixed)]);
    assert_eq!(error.kind, InputErrorKind::Binding);
    assert!(error.path.unwrap().ends_with("mixed.ts"));
    assert_eq!(error.position, Some(at(2, 18)));
    // Dependency-only declarations are lenient regardless of skipLibCheck.
    for config in [CONFIG, SKIP_LIB_CHECK] {
        let fixture = Fixture::new(&[
            ("tsconfig.json", config),
            (
                "node_modules/pkg/package.json",
                r#"{ "name": "pkg", "types": "index.d.ts" }"#,
            ),
            ("node_modules/pkg/index.d.ts", mixed),
            (
                "app.ts",
                "import { /*@f:i*/f } from \"pkg\";\n/*@f:u*/f();\n",
            ),
        ]);
        fixture.check_markers(&fixture.load());
    }
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
fn comment_directives_count_between_any_tokens_but_not_inside_literals() {
    let fixture = project(&[(
        "app.ts",
        "import {\n  /*@?*/a,\n  // @ts-expect-error The module is generated later.\n} from \"./generated\";\nexport const /*@x:de*/x = /*@?*/a;\n",
    )]);
    fixture.check_markers(&fixture.load());

    let error = load_error(&[(
        "literal.ts",
        "export const t = `\n// @ts-ignore`;\nimport { b } from \"./absent\";\n",
    )]);
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport);
    assert_eq!(error.position, Some(at(3, 19)));
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

    // Past the budget, loading fails with a resource limit, not a syntax error.
    let error = load_error(&[("hostile.ts", &deep(5_000))]);
    assert_eq!(error.kind, InputErrorKind::ResourceLimit);
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
fn name_candidates_that_are_not_reference_targets_fail_the_query() {
    let fixture = project(&[
        (
            "path.d.ts",
            "declare namespace path { interface PlatformPath { resolve(): string } }\ndeclare const path: path.PlatformPath;\nexport = path;\n",
        ),
        (
            "app.ts",
            "import { resolve } from \"./path\";\nexport { globalThis as g };\n",
        ),
    ]);
    let project = fixture.load();
    let error = |name: &str| {
        project
            .find_references(ReferenceQuery::Name {
                name: name.into(),
                file: None,
            })
            .query_error()
    };
    let resolve = error("resolve");
    assert_eq!(resolve.kind, QueryErrorKind::UnsupportedTarget);
    assert!(resolve.path.unwrap().ends_with("app.ts"));
    assert_eq!(resolve.position, Some(at(1, 10)));
    assert_eq!(error("g").kind, QueryErrorKind::UnsupportedTarget);
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
            "import /*@?*/D from \"./lib\";\n/*@?*/D./*@!*/x;\n/*@?*/D./*@!*/x./*@!*/more;\ntype /*@Qualified:d*/Qualified = /*@?*/D./*@?*/x;\nexport {};\n",
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
    let error = Project::load(&missing.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport);
}

#[cfg(unix)]
#[test]
fn a_symlinked_configuration_directory_does_not_redirect_parent_paths() {
    // `L/x` links to `real/deep/x`. Configuration paths resolve from the real
    // directory, where `../z.ts` is `real/deep/z.ts`; from the link, the same
    // spelling would lexically name `L/z.ts`.
    let fixture = Fixture::new(&[
        (
            "real/deep/x/tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["../../../L/z.ts", "../z.ts", "in.ts"] }"#,
        ),
        ("real/deep/x/in.ts", "export const inside = 1;\n"),
        ("L/z.ts", "export const near = 1;\n"),
        ("real/deep/z.ts", "export const far = 1;\n"),
    ]);
    std::os::unix::fs::symlink(fixture.path("real/deep/x"), fixture.path("L/x")).unwrap();
    let project = Project::load(&fixture.path("L/x/tsconfig.json")).unwrap();
    let query = |file| {
        project
            .find_references(ReferenceQuery::At {
                file,
                position: at(1, 14),
            })
            .unwrap()
    };
    let name = |result: &typescript_parser::references::ReferenceResult| {
        result.groups[0].declarations[0].name.clone()
    };
    assert_eq!(name(&query(fixture.path("L/z.ts"))), "near");
    assert_eq!(name(&query(fixture.path("real/deep/z.ts"))), "far");
    // A file under the configuration directory can be named through the link.
    assert_eq!(name(&query(fixture.path("L/x/in.ts"))), "inside");
    // Result files are relative to the reported configuration's directory.
    let result = query(fixture.path("real/deep/z.ts"));
    assert_eq!(result.project, fixture.path("real/deep/x/tsconfig.json"));
    let file = &result.groups[0].declarations[0].location.file;
    assert_eq!(
        std::fs::canonicalize(result.project.parent().unwrap().join(file)).unwrap(),
        fixture.path("real/deep/z.ts")
    );
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
fn input_extensions_follow_the_file_system_case_policy() {
    let fixture = project(&[
        ("script.js", "export const s = 1;\n"),
        ("types.ts", "declare const t: number;\n"),
        (
            "app.ts",
            "/// <reference path=\"./TYPES.TS\" />\nexport const a = t;\n",
        ),
    ]);
    // Only a case-insensitive file system has other spellings of a name.
    if !fixture.path("SCRIPT.JS").exists() {
        eprintln!(
            "SKIP input_extensions_follow_the_file_system_case_policy: case-sensitive file system"
        );
        return;
    }
    let configure = |files: &str| {
        let config = format!(r#"{{ "compilerOptions": {{ "noLib": true }}, "files": {files} }}"#);
        std::fs::write(fixture.path("tsconfig.json"), config).unwrap();
        Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error)
    };
    let error = configure(r#"["SCRIPT.JS"]"#).unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
    let project = configure(r#"["app.ts"]"#).unwrap_or_else(|error| panic!("{error}"));
    let result = project
        .find_references(ReferenceQuery::Name {
            name: "t".into(),
            file: None,
        })
        .unwrap();
    assert_eq!(result.groups[0].occurrences.len(), 2);
}

#[test]
fn declaration_files_are_named_by_their_exact_spelling() {
    // tsc 7.0.2 reports TS2304 for `g`: `G.D.TS` is an ordinary module under
    // `nodenext`, not a declaration file, though the file system ignores case.
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext" }, "files": ["main.ts", "G.D.TS"] }"#,
        ),
        (
            "main.ts",
            "export {};
g;
",
        ),
        (
            "G.D.TS",
            "declare const g: number;
",
        ),
    ]);
    if !fixture.path("g.d.ts").exists() {
        eprintln!(
            "SKIP declaration_files_are_named_by_their_exact_spelling: case-sensitive file system"
        );
        return;
    }
    let error = fixture
        .load()
        .find_references(ReferenceQuery::At {
            file: fixture.path("main.ts"),
            position: SourcePosition { line: 2, column: 1 },
        })
        .query_error();
    assert_eq!(error.kind, QueryErrorKind::UnresolvedTarget, "{error}");
}

#[test]
fn equally_specific_pattern_ambient_modules_resolve_to_the_first() {
    // As in tsc 7.0.2 (`findBestPatternMatch`): both patterns have an empty
    // prefix, so the first declared wins; a name with two `*` is no pattern.
    let client = "declare module '*.module.css' { export const onlyModule: number; }\n\
                  declare module '*.css' { export const onlyPlain: number; }\n\
                  declare module 'a*b*c' { export const twoStars: number; }\n";
    let resolve = |import: &str| {
        let fixture = project(&[
            ("client.d.ts", client),
            (
                "app.ts",
                &format!("import {{ {import} }} from \"./x.module.css\";\n{import};\n"),
            ),
        ]);
        fixture.load().find_references(ReferenceQuery::At {
            file: fixture.path("app.ts"),
            position: SourcePosition {
                line: 1,
                column: 10,
            },
        })
    };
    let result = resolve("onlyModule").unwrap();
    assert_eq!(
        result.groups[0].declarations[0].location.file,
        "client.d.ts"
    );
    assert_eq!(result.groups[0].declarations[0].location.start.line, 1);
    let error = resolve("onlyPlain").query_error();
    assert_eq!(error.kind, QueryErrorKind::UnresolvedTarget, "{error}");

    let fixture = project(&[
        ("client.d.ts", client),
        ("app.ts", "import { twoStars } from \"aZb*c\";\ntwoStars;\n"),
    ]);
    let error = Project::load(&fixture.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
}

#[test]
fn equally_specific_patterns_in_different_files_follow_program_order() {
    // TypeScript collects pattern ambient modules file by file in program
    // order, which lists `b.d.ts`, referenced from the first root, before
    // the second root `c.d.ts`; both patterns have an empty prefix, so
    // `b.d.ts`'s `"*s"` wins for `x.css`, as tsc 7.0.2's groups show.
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["a.ts", "c.d.ts"] }"#,
        ),
        (
            "a.ts",
            "/// <reference path=\"./b.d.ts\" />\nimport { /*@X:i*/X } from \"x.css\";\nexport let /*@x:de*/x: /*@X:u*/X;\n",
        ),
        (
            "b.d.ts",
            "declare module \"*s\" { interface /*@X:d*/X { /*@!*/s: number } }\n",
        ),
        (
            "c.d.ts",
            "declare module \"*.css\" { interface /*@Y:d*/X { /*@!*/css: number } }\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn a_star_export_route_that_fails_still_exports_its_name() {
    // `b.ts` re-exports a missing `x`. tsc 7.0.2 reports TS2305 there and,
    // beside `c.ts`, TS2308 at the second `export *`: the failing route still
    // exports `x`, so the name is ambiguous rather than `c.ts`'s.
    let query = |barrel: &str| {
        let fixture = project(&[
            ("a.ts", barrel),
            ("b.ts", "export { x } from \"./c2.js\";\n"),
            ("c.ts", "export const x = 1;\n"),
            ("c2.ts", "export const y = 1;\n"),
            ("main.ts", "import { x } from \"./a.js\";\nx;\n"),
        ]);
        fixture
            .load()
            .find_references(ReferenceQuery::At {
                file: fixture.path("main.ts"),
                position: SourcePosition {
                    line: 1,
                    column: 10,
                },
            })
            .query_error()
    };
    let error = query("export * from \"./b.js\";\nexport * from \"./c.js\";\n");
    assert_eq!(error.kind, QueryErrorKind::UnresolvedTarget, "{error}");
    assert!(error.message.contains("ambiguous"), "{error}");
    let error = query("export * from \"./b.js\";\n");
    assert_eq!(error.kind, QueryErrorKind::UnresolvedTarget, "{error}");
    assert!(error.message.contains("not exported"), "{error}");
}

#[test]
fn exported_and_local_declarations_conflict_by_declaration_space() {
    // TypeScript's `getDeclarationSpaces` (TS2395): an enum occupies the
    // value and type spaces only, so it merges with a local type-only
    // namespace but not with a local instantiated one.
    let load = |source: &str| {
        let fixture = project(&[("a.ts", source)]);
        Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error)
    };
    for source in [
        "export enum Color { Red }\nnamespace Color { export type Name = keyof typeof Color; }\n",
        "enum Color { Red }\nexport namespace Color { export type Name = string; }\n",
    ] {
        load(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
    for source in [
        "export enum Color { Red }\nnamespace Color { export const x = 1; }\n",
        "export class C {}\nnamespace C { export const x = 1; }\n",
        "export class C {}\ninterface C { x: number }\n",
    ] {
        let error = load(source).unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Binding, "{source}: {error}");
    }
}

#[test]
fn a_const_enum_namespace_precedes_a_function_unless_const_enums_are_kept() {
    // TypeScript's `isInstantiatedModule` counts a namespace of const enums
    // only under `shouldPreserveConstEnums` (TS2434 otherwise).
    let source = "namespace f { export const enum E { A } }\nfunction f() {}\nexport {};\n";
    let load = |options: &str| {
        let config = format!(r#"{{ "compilerOptions": {{ "noLib": true{options} }} }}"#);
        let fixture = Fixture::new(&[("tsconfig.json", config.as_str()), ("a.ts", source)]);
        Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error)
    };
    load("").unwrap_or_else(|error| panic!("{error}"));
    for option in [
        "isolatedModules",
        "preserveConstEnums",
        "verbatimModuleSyntax",
    ] {
        let error = load(&format!(r#", "{option}": true"#)).unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Binding, "{option}: {error}");
    }
    let instantiated = "namespace f { export const x = 1; }\nfunction f() {}\nexport {};\n";
    let fixture = project(&[("a.ts", instantiated)]);
    let error = Project::load(&fixture.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Binding, "{error}");
}

#[test]
fn relative_specifiers_accept_backslash_separators() {
    // tsc 7.0.2 resolves each of these to `sub/a.ts`: `pathIsRelative` and
    // `combinePaths` read `\\` as a separator.
    for specifier in [".\\\\sub\\\\a", "./sub\\\\a", ".\\\\sub/a"] {
        let fixture = project(&[
            ("sub/a.ts", "export const value = 1;\n"),
            (
                "main.ts",
                &format!("import {{ value }} from \"{specifier}\";\nvalue;\n"),
            ),
        ]);
        let result = fixture
            .load()
            .find_references(ReferenceQuery::Name {
                name: "value".into(),
                file: None,
            })
            .unwrap_or_else(|error| panic!("{specifier}: {error}"));
        assert_eq!(result.groups.len(), 1, "{specifier}");
        assert_eq!(result.groups[0].occurrences.len(), 3, "{specifier}");
    }
}

#[test]
fn both_entry_points_report_a_failing_token_at_the_same_path() {
    let fixture = project(&[
        ("src/m.ts", "export const a = 1;\n"),
        ("src/c.ts", "import { b } from \"./m\";\nb;\n"),
    ]);
    let project = fixture.load();
    let at = project
        .find_references(ReferenceQuery::At {
            file: fixture.root.join("src/../src/c.ts"),
            position: SourcePosition {
                line: 1,
                column: 10,
            },
        })
        .query_error();
    let named = project
        .find_references(ReferenceQuery::Name {
            name: "b".into(),
            file: None,
        })
        .query_error();
    assert_eq!(at.kind, QueryErrorKind::UnresolvedTarget, "{at}");
    assert_eq!(named.kind, QueryErrorKind::UnresolvedTarget, "{named}");
    assert_eq!(at.path, named.path);
    assert!(at.path.as_ref().unwrap().is_absolute());
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
    let error = Project::load(&fixture.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport);
}

#[test]
fn long_lines_convert_positions_in_linear_time() {
    // `é` is one UTF-16 unit and two UTF-8 bytes.
    let source = format!("export const x = \"é\";{}\n", "x;".repeat(40_000));
    let fixture = project(&[("app.ts", &source)]);
    let config = fixture.path("tsconfig.json");
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let project = Project::load(&config).unwrap();
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
        send.send(Project::load(&config).is_ok()).unwrap();
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
        assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{directive}");
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
        assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{directive}");
    }
}

/// An unresolved module or reference directive target fails the load only
/// where it is reported (`fixtures/json-probes/reporting.json`): in a
/// project file, unless skipLibCheck covers it as a declaration file. A
/// package's are never reported here.
#[test]
fn unresolved_inputs_fail_only_where_reported() {
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
                let result =
                    Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error);
                let tolerated = !local || skip && file.ends_with(".d.ts");
                match result {
                    Ok(_) => assert!(tolerated, "{name} {file} local={local} skip={skip} loaded"),
                    Err(error) => {
                        assert!(
                            !tolerated,
                            "{name} {file} local={local} skip={skip}: {error}"
                        );
                        assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
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
        Project::load(&fixture.path("tsconfig.json"))
            .map_err(support::input_error)
            .map(|_| ())
    };
    // Unchecked side-effect imports do not include the runtime.
    let error = load(false, "export const y = <div />;\n").unwrap_err();
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport);
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
        Project::load(&fixture.path("tsconfig.json"))
            .map_err(support::input_error)
            .map(|_| ())
    };
    load("// @jsxImportSource preact\nexport const e = <div />;\n").unwrap();
    for pragma in [
        "/* @JSXIMPORTSOURCE preact */",
        "/**\n * @jsxImportSource preact\n */",
    ] {
        let error = load(&format!("{pragma}\nexport const e = <div />;\n")).unwrap_err();
        assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{pragma}");
    }
}

#[test]
fn suppression_directives_are_real_comments_on_preceding_lines() {
    // A directive inside a block comment is not a directive.
    let error = load_error(&[(
        "app.ts",
        "/*\n// @ts-ignore\n*/ import { x } from \"./missing\";\nexport const y = 1;\n",
    )]);
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport);
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
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport);
}

#[test]
fn alias_errors_are_reported_at_the_alias_name_or_its_keyword() {
    let duplicate = |file: &str, source: &str| {
        let error = load_error(&[("lib.ts", "export const x = 1;\n"), (file, source)]);
        assert_eq!(error.kind, InputErrorKind::Binding, "{source}: {error}");
        assert!(error.message.contains("duplicate"), "{source}: {error}");
        error.position
    };
    // Nameless entity aliases are anchored at `default` or `=`.
    let entity = "declare namespace a { const b: number; }\n";
    assert_eq!(
        duplicate(
            "app.ts",
            &format!("{entity}export default a.b;\nexport default a.b;\n")
        ),
        Some(at(3, 8))
    );
    assert_eq!(
        duplicate("app.ts", &format!("{entity}export = a.b;\nexport = a.b;\n")),
        Some(at(3, 8))
    );
    assert_eq!(
        duplicate(
            "app.d.ts",
            "export declare const x: number;\nexport as namespace X;\nexport as namespace X;\n"
        ),
        Some(at(3, 21))
    );
    // A string-literal export name anchors its alias.
    assert_eq!(
        duplicate(
            "app.ts",
            "export * as \"s\" from \"./lib\";\nexport * as \"s\" from \"./lib\";\n"
        ),
        Some(at(2, 13))
    );
}

#[test]
fn a_decorator_cannot_begin_an_object_literal_member() {
    // TypeScript 7.0.2 reports TS1136 at the `@`: an object literal member
    // starts only with `[`, `*`, `...`, `.`, or a literal property name.
    let error = load_error(&[(
        "app.ts",
        "declare function dec(t: any): any;\nexport const o = { @dec a: 1 };\n",
    )]);
    assert_eq!(error.kind, InputErrorKind::Syntax, "{error}");
    assert_eq!(error.message, "Property assignment expected");
    assert_eq!(error.position, Some(at(2, 20)));
}

#[test]
fn identifiers_without_a_binding_rule_name_their_construct() {
    // TypeScript 7.0.2 parses decorators after a member's modifier
    // (`parseModifiersEx`) and reports only TS1042, a checker grammar error
    // (`fixtures/json-probes/parsing.json`, `decorator-after-export-modifier`).
    // No binding rule covers such a decorator, so the binder's coverage check
    // rejects the file instead of indexing it without the identifier.
    let error = load_error(&[(
        "app.ts",
        "declare function dec(t: any): any;\nexport const o = { export @dec a: 1 };\n",
    )]);
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
    assert_eq!(
        error.message,
        "unsupported identifier in Decorator (Identifier)"
    );
    assert_eq!(error.position, Some(at(2, 28)));
}

#[test]
fn augmentations_of_scripts_fail_only_where_bindings_are_checked() {
    // TypeScript reports TS2306 at the augmented name.
    let error = load_error(&[
        ("globals.ts", "var pluginGlobal = 1;\n"),
        (
            "app.ts",
            "export {};\ndeclare module \"./globals\" { export interface Options { extra: string } }\n",
        ),
    ]);
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport);
    assert_eq!(error.position, Some(at(2, 16)));

    // A package's augmentation of a script binds nothing.
    let fixture = Fixture::new(&[
        ("tsconfig.json", SKIP_LIB_CHECK),
        (
            "node_modules/plugin/package.json",
            r#"{ "name": "plugin", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/plugin/index.d.ts",
            "/// <reference path=\"./globals.d.ts\" />\nexport declare const plugin: number;\ndeclare module \"./globals\" { export interface Options { extra: string } }\n",
        ),
        (
            "node_modules/plugin/globals.d.ts",
            "interface Options { base: string }\n",
        ),
        (
            "app.ts",
            "import { /*@plugin:i*/plugin } from \"plugin\";\n/*@plugin:u*/plugin;\ndeclare let /*@o:d*/o: /*@Options:u*/Options;\n/*@o:u*/o;\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

#[test]
fn qualified_aliases_keep_type_only_values_for_type_queries() {
    // TypeScript reports TS1379 at the alias but resolves `typeof x` to `C`.
    let fixture = project(&[
        ("m.ts", "class /*@C:d*/C {}\nexport type { /*@C:e*/C };\n"),
        (
            "app.ts",
            "import * as /*@m:i*/ns from \"./m\";\nimport /*@C:i*/x = /*@m:u*/ns./*@C:u*/C;\ntype /*@T:d*/T = typeof /*@C:u*/x;\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

/// TypeScript 7.0.2 accepts this project: a module object is a value and a
/// namespace, and so is an `import =` alias of one. The fixture
/// `module-aliases` checks more forms against the semantic oracle.
#[test]
fn import_equals_aliases_of_module_objects_are_values() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext" } }"#,
        ),
        ("m.ts", "export function foo() {}\n"),
        (
            "a.ts",
            "import * as ns from \"./m.js\";\nimport x = ns;\nx.foo();\n",
        ),
    ]);
    let result = fixture
        .load()
        .find_references(ReferenceQuery::At {
            file: fixture.path("a.ts"),
            position: at(3, 1),
        })
        .unwrap();
    assert_eq!(result.groups.len(), 1);
    let group = &result.groups[0];
    assert_eq!(group.declarations.len(), 1);
    assert_eq!(group.declarations[0].name, "m.ts");
    let occurrences: Vec<_> = group
        .occurrences
        .iter()
        .map(|occurrence| (occurrence.name.as_str(), occurrence.location.start))
        .collect();
    assert_eq!(
        occurrences,
        [
            ("ns", at(1, 13)),
            ("x", at(2, 8)),
            ("ns", at(2, 12)),
            ("x", at(3, 1))
        ]
    );
}

#[test]
fn module_alias_meanings_keep_shorthand_members_and_type_only_routes() {
    // An export of a shorthand ambient module has every meaning a use asks
    // for, so the named import is also a type. The module object, which an
    // `import =` alias expands, is only a value and a namespace; TypeScript
    // reports TS2709 at `let u: y`.
    let shorthand = project(&[
        ("shorty.d.ts", "declare module \"shorty\";\n"),
        (
            "app.ts",
            "import { /*@m:i*/thing } from \"shorty\";\nimport * as /*@m:i*/ns from \"shorty\";\nimport /*@m:i*/y = /*@m:u*/thing;\nimport /*@m:i*/z = /*@m:u*/ns;\nlet /*@t:d*/t: /*@m:u*/thing;\nlet /*@u:d*/u: /*@?*/y;\n/*@m:u*/y(); /*@m:u*/z(); /*@m:u*/z./*@?*/member;\n/*@t:u*/t; /*@u:u*/u;\nexport {};\n",
        ),
    ]);
    shorthand.check_markers(&shorthand.load());

    // An alias of a type-only module route keeps that route's restriction on
    // the value meaning it adds. TypeScript reports TS1380 and TS1379 at the
    // aliases and TS1361 and TS1362 at the value uses.
    let type_only = project(&[
        (
            "m.ts",
            "export function /*@foo:de*/foo() {}\nexport interface /*@I:de*/I {}\n",
        ),
        (
            "re.ts",
            "import * as /*@m:i*/ns from \"./m\";\nexport type { /*@m:e*/ns };\n",
        ),
        (
            "app.ts",
            "import type * as /*@m:i*/ns from \"./m\";\nimport { /*@m:i*/ns as /*@m:i*/exported } from \"./re\";\nimport /*@m:i*/x = /*@m:u*/ns;\nimport /*@m:i*/y = /*@m:u*/exported;\n/*@?*/x./*@!*/foo(); /*@?*/y./*@!*/foo();\nlet /*@a:d*/a: typeof /*@m:u*/x./*@foo:u*/foo;\nlet /*@b:d*/b: typeof /*@m:u*/y./*@foo:u*/foo;\nlet /*@c:d*/c: /*@m:u*/x./*@I:u*/I = /*@a:u*/a;\nlet /*@d:d*/d: /*@m:u*/y./*@I:u*/I = /*@b:u*/b;\n/*@c:u*/c; /*@d:u*/d;\n",
        ),
    ]);
    type_only.check_markers(&type_only.load());
}

/// A dependency's declarations merge leniently where they conflict, so a
/// namespace wrongly counted as a value would silently join the constant.
#[test]
fn uninstantiated_namespaces_in_dependencies_stay_beside_values() {
    let fixture = Fixture::new(&[
        ("tsconfig.json", SKIP_LIB_CHECK),
        (
            "node_modules/pkg/package.json",
            r#"{ "name": "pkg", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/pkg/index.d.ts",
            "export declare namespace A { namespace B { type T = 1; } export { B }; }\nexport declare const A: number;\n",
        ),
        ("app.ts", "import { A } from \"pkg\";\nA;\n"),
    ]);
    let result = fixture
        .load()
        .find_references(ReferenceQuery::At {
            file: fixture.path("app.ts"),
            position: at(2, 1),
        })
        .unwrap();
    assert_eq!(result.groups.len(), 1);
    let declarations = &result.groups[0].declarations;
    assert_eq!(declarations.len(), 1, "{declarations:?}");
    assert_eq!(declarations[0].location.start, at(2, 22));
}

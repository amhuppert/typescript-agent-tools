//! Module augmentation identity. TypeScript 7.0.2's `mergeModuleAugmentation`
//! merges an augmentation's declarations into the symbols the augmented module
//! exports: through an export alias to its target, through `export *` to the
//! name the stars supply, into an `export =` namespace, recursively into the
//! namespaces they declare, and one way into a pattern ambient module for the
//! augmented specifier only. Expected groups, including package declarations,
//! were observed with the pinned 7.0.2 semantic oracle
//! (`tools/semantic-oracle.mjs`); expected failures are the diagnostics its
//! native `tsc` reports where bindings are checked. Documented differences
//! are fixtures in `fixtures/reference-exceptions` (`reference_exceptions.rs`).

mod support;

use support::{Expect, Fixture};
use typescript_parser::analysis::{InputError, InputErrorKind, QueryErrorKind};
use typescript_parser::project::Project;
use typescript_parser::references::{Location, ReferenceQuery};
use typescript_parser::source::SourcePosition;

const MANIFEST: &str = r#"{ "name": "pkg", "types": "index.d.ts" }"#;

/// A project of `main.ts` and `others`, which imports package `pkg` built
/// from `package` files (paths relative to `node_modules/pkg`).
fn with_package(package: &[(&str, &str)], main: &str, others: &[(&str, &str)]) -> Fixture {
    let mut roots = vec!["\"main.ts\"".to_string()];
    roots.extend(others.iter().map(|(path, _)| format!("\"{path}\"")));
    let config = format!(
        r#"{{ "compilerOptions": {{ "noLib": true }}, "files": [{}] }}"#,
        roots.join(", ")
    );
    let package: Vec<(String, &str)> = package
        .iter()
        .map(|(path, text)| (format!("node_modules/pkg/{path}"), *text))
        .collect();
    let mut files = vec![
        ("tsconfig.json", config.as_str()),
        ("node_modules/pkg/package.json", MANIFEST),
        ("main.ts", main),
    ];
    files.extend(package.iter().map(|(path, text)| (path.as_str(), *text)));
    files.extend_from_slice(others);
    Fixture::new(&files)
}

fn load(fixture: &Fixture, case: &str) -> Project {
    Project::load(&fixture.path("tsconfig.json"))
        .unwrap_or_else(|error| panic!("{case}: project failed to load: {error}"))
}

fn load_error(fixture: &Fixture, case: &str) -> InputError {
    match Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error) {
        Ok(_) => panic!("{case}: the project should fail to load"),
        Err(error) => error,
    }
}

fn at(line: usize, column: usize) -> SourcePosition {
    SourcePosition { line, column }
}

/// The declarations under `node_modules` of the group marked `label`, as
/// `file:line:column`. Marker checks cover searched files; these
/// declarations are in files whose occurrences are not reported.
fn package_declarations(fixture: &Fixture, project: &Project, label: &str) -> Vec<String> {
    let markers = fixture.markers();
    let marked: Vec<_> = markers
        .iter()
        .filter(|marker| {
            matches!(&marker.expect, Expect::Groups { labels, .. } if labels.iter().any(|l| l == label))
        })
        .map(|marker| marker.location.clone())
        .collect();
    let first = marked
        .first()
        .unwrap_or_else(|| panic!("no token is marked `{label}`"));
    let result = project
        .find_references(ReferenceQuery::At {
            file: fixture.path(&first.file),
            position: first.start,
        })
        .unwrap();
    let group = result
        .groups
        .iter()
        .find(|group| {
            group
                .occurrences
                .iter()
                .map(|occurrence| &occurrence.location)
                .eq(marked.iter())
        })
        .unwrap_or_else(|| panic!("no group has exactly the tokens marked `{label}`"));
    group
        .declarations
        .iter()
        .filter(|declaration| declaration.location.file.starts_with("node_modules/"))
        .map(|declaration| {
            let start = declaration.location.start;
            format!(
                "{}:{}:{}",
                declaration.location.file, start.line, start.column
            )
        })
        .collect()
}

/// A group as its declarations and occurrences, each `file:line:column`.
type Group = (Vec<String>, Vec<String>);

fn group(declarations: &[&str], occurrences: &[&str]) -> Group {
    let owned = |places: &[&str]| places.iter().map(|place| place.to_string()).collect();
    (owned(declarations), owned(occurrences))
}

/// The groups a position query at `file:line:column` returns.
fn groups_at(fixture: &Fixture, project: &Project, place: &str) -> Vec<Group> {
    let mut parts = place.rsplitn(3, ':');
    let column = parts.next().unwrap().parse().unwrap();
    let line = parts.next().unwrap().parse().unwrap();
    let file = parts.next().unwrap();
    let result = project
        .find_references(ReferenceQuery::At {
            file: fixture.path(file),
            position: at(line, column),
        })
        .unwrap_or_else(|error| panic!("query at {place}: {error}"));
    let place = |location: &Location| {
        format!(
            "{}:{}:{}",
            location.file, location.start.line, location.start.column
        )
    };
    result
        .groups
        .iter()
        .map(|group| {
            (
                group
                    .declarations
                    .iter()
                    .map(|d| place(&d.location))
                    .collect(),
                group
                    .occurrences
                    .iter()
                    .map(|o| place(&o.location))
                    .collect(),
            )
        })
        .collect()
}

/// Every query at one of `places` returns exactly `expected`.
fn expect_groups(fixture: &Fixture, project: &Project, places: &[&str], expected: &[Group]) {
    for place in places {
        assert_eq!(
            groups_at(fixture, project, place),
            expected,
            "query at {place}"
        );
    }
}

/// The TipTap shape: a generic interface augmented through what `pkg` exports.
const AUGMENT: &str = r#"import type { /*@C:i*/Commands } from "pkg";
declare module "pkg" {
  interface /*@C:d*/Commands</*@T:d*/T> { /*@!*/custom: /*@T:u*/T; }
}
export declare const /*@c:de*/c: /*@C:u*/Commands<number>;
export const /*@n:de*/n: number = /*@c:u*/c./*@!*/custom;
"#;

/// A case: its name, the package's files, and the package declarations of
/// `Commands` and of its type parameter.
type Case<'a> = (&'a str, &'a [(&'a str, &'a str)], &'a str, &'a str);

#[test]
fn augmentations_merge_with_what_the_augmented_export_names() {
    // Each export form, with the package declarations of `Commands` and its
    // type parameter that the oracle groups with the augmentation's.
    let cases: &[Case] = &[
        (
            "direct export",
            &[("index.d.ts", "export interface Commands<T = any> {}\n")],
            "node_modules/pkg/index.d.ts:1:18",
            "node_modules/pkg/index.d.ts:1:27",
        ),
        (
            "export type specifier",
            &[(
                "index.d.ts",
                "interface Commands<T = any> {}\nexport type { Commands };\n",
            )],
            "node_modules/pkg/index.d.ts:1:11",
            "node_modules/pkg/index.d.ts:1:20",
        ),
        (
            "export specifier",
            &[(
                "index.d.ts",
                "interface Commands<T = any> {}\nexport { Commands };\n",
            )],
            "node_modules/pkg/index.d.ts:1:11",
            "node_modules/pkg/index.d.ts:1:20",
        ),
        (
            "renamed export",
            &[(
                "index.d.ts",
                "interface Inner<T = any> {}\nexport type { Inner as Commands };\n",
            )],
            "node_modules/pkg/index.d.ts:1:11",
            "node_modules/pkg/index.d.ts:1:17",
        ),
        (
            "re-export from another module",
            &[
                (
                    "index.d.ts",
                    "export type { Commands } from \"./commands\";\n",
                ),
                ("commands.d.ts", "export interface Commands<T = any> {}\n"),
            ],
            "node_modules/pkg/commands.d.ts:1:18",
            "node_modules/pkg/commands.d.ts:1:27",
        ),
        (
            "re-exported import",
            &[
                (
                    "index.d.ts",
                    "import { Commands } from \"./commands\";\nexport { Commands };\n",
                ),
                ("commands.d.ts", "export interface Commands<T = any> {}\n"),
            ],
            "node_modules/pkg/commands.d.ts:1:18",
            "node_modules/pkg/commands.d.ts:1:27",
        ),
        (
            "export *",
            &[
                ("index.d.ts", "export * from \"./commands\";\n"),
                ("commands.d.ts", "export interface Commands<T = any> {}\n"),
            ],
            "node_modules/pkg/commands.d.ts:1:18",
            "node_modules/pkg/commands.d.ts:1:27",
        ),
        (
            "export = namespace",
            &[(
                "index.d.ts",
                "declare namespace pkg { interface Commands<T = any> {} }\nexport = pkg;\n",
            )],
            "node_modules/pkg/index.d.ts:1:35",
            "node_modules/pkg/index.d.ts:1:44",
        ),
        (
            "class through an export type specifier",
            &[(
                "index.d.ts",
                "declare class Commands<T = any> {}\nexport type { Commands };\n",
            )],
            "node_modules/pkg/index.d.ts:1:15",
            "node_modules/pkg/index.d.ts:1:24",
        ),
    ];
    for &(case, package, commands, parameter) in cases {
        let fixture = with_package(package, AUGMENT, &[]);
        let project = load(&fixture, case);
        fixture.check_markers(&project);
        assert_eq!(
            package_declarations(&fixture, &project, "C"),
            [commands],
            "{case}"
        );
        assert_eq!(
            package_declarations(&fixture, &project, "T"),
            [parameter],
            "{case}"
        );
    }
}

#[test]
fn every_augmentation_joins_one_identity_with_the_package_self_augmentation() {
    // TipTap augments its own `Commands` before declaring it, then exports it
    // with an element-level type-only specifier; each plugin augments it
    // again. The oracle groups all of them.
    let package = [(
        "index.d.ts",
        "declare module \"pkg\" {\n    interface Commands<T> { blur: T }\n}\ninterface Commands<T = any> {}\nexport { type Commands };\n",
    )];
    let other = "export {};\ndeclare module \"pkg\" {\n  interface /*@C:d*/Commands</*@T:d*/T> { /*@!*/other: /*@T:u*/T; }\n}\n";
    let fixture = with_package(&package, AUGMENT, &[("other.ts", other)]);
    let project = load(&fixture, "self augmentation");
    fixture.check_markers(&project);
    let both = [
        "node_modules/pkg/index.d.ts:2:15",
        "node_modules/pkg/index.d.ts:4:11",
    ];
    assert_eq!(package_declarations(&fixture, &project, "C"), both);
    let both = [
        "node_modules/pkg/index.d.ts:2:24",
        "node_modules/pkg/index.d.ts:4:20",
    ];
    assert_eq!(package_declarations(&fixture, &project, "T"), both);
}

#[test]
fn augmentations_through_an_alias_merge_across_project_files() {
    // The original declaration, its export specifier, the augmentation, and
    // consumers, including a renamed import, form one group; the merged
    // interfaces share their type parameter.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "api.ts",
            "interface /*@C:d*/Commands</*@T:d*/T = any> { /*@!*/base: /*@T:u*/T }\nexport type { /*@C:e*/Commands };\nexport declare const /*@local:de*/local: /*@C:u*/Commands;\n",
        ),
        (
            "main.ts",
            "import type { /*@C:i*/Commands } from \"./api\";\ndeclare module \"./api\" {\n  interface /*@C:d*/Commands</*@T:d*/T> { /*@!*/custom: /*@T:u*/T; }\n}\nexport declare const /*@c:de*/c: /*@C:u*/Commands<number>;\n",
        ),
        (
            "use.ts",
            "import type { /*@C:i*/Commands as /*@C:i*/C } from \"./api\";\nexport declare const /*@u:de*/u: /*@C:u*/C<string>;\nexport const /*@s:de*/s: string = /*@u:u*/u./*@!*/custom;\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "project files"));
}

#[test]
fn merges_through_aliases_follow_the_declaration_meanings() {
    // A class augmentation gives an aliased interface a value, which the
    // original module's own uses and every importer see. The merge replaces
    // the type-only export with the merged symbol, so a value use through it
    // resolves; without the augmentation it does not (TS1362).
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "api.ts",
            "interface /*@C:d*/Commands { /*@!*/base: number }\nexport type { /*@C:e*/Commands };\nexport const /*@made:de*/made = new /*@C:u*/Commands();\n",
        ),
        (
            "aug.ts",
            "export {};\ndeclare module \"./api\" {\n  class /*@C:d*/Commands { /*@!*/extra: number }\n}\n",
        ),
        (
            "use.ts",
            "import { /*@C:i*/Commands, /*@made:i*/made } from \"./api\";\nexport const /*@used:de*/used = new /*@C:u*/Commands();\nexport const /*@sum:de*/sum: number = /*@used:u*/used./*@!*/base + /*@made:u*/made./*@!*/extra;\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "class adds a value"));

    let package = [(
        "index.d.ts",
        "declare class Commands<T = any> {}\nexport type { Commands };\n",
    )];
    let merged = "import { /*@C:i*/Commands } from \"pkg\";\ndeclare module \"pkg\" {\n  interface /*@C:d*/Commands</*@T:d*/T> { /*@!*/custom: /*@T:u*/T; }\n}\nexport const /*@x:de*/x = new /*@C:u*/Commands<number>();\n";
    let fixture = with_package(&package, merged, &[]);
    let project = load(&fixture, "merged type-only class");
    fixture.check_markers(&project);
    assert_eq!(
        package_declarations(&fixture, &project, "C"),
        ["node_modules/pkg/index.d.ts:1:15"]
    );
    let unmerged = "import { /*@C:i*/Commands } from \"pkg\";\nexport const /*@x:de*/x = new /*@?*/Commands<number>();\n";
    let fixture = with_package(&package, unmerged, &[]);
    fixture.check_markers(&load(&fixture, "type-only class"));

    // Declarations of other meanings stay separate symbols, as the oracle's
    // per-meaning groups show: an interface beside an aliased constant, and a
    // constant beside an aliased interface.
    let package = [(
        "index.d.ts",
        "declare const Commands: number;\nexport { Commands };\n",
    )];
    let main = "import type { /*@C,V:i*/Commands } from \"pkg\";\ndeclare module \"pkg\" {\n  interface /*@C:d*/Commands</*@T:d*/T> { /*@!*/custom: /*@T:u*/T; }\n}\nexport declare const /*@c:de*/c: /*@C:u*/Commands<number>;\n";
    let fixture = with_package(&package, main, &[]);
    let project = load(&fixture, "interface beside a constant");
    fixture.check_markers(&project);
    assert_eq!(
        package_declarations(&fixture, &project, "V"),
        ["node_modules/pkg/index.d.ts:1:15"]
    );
    let package = [(
        "index.d.ts",
        "interface Commands<T = any> {}\nexport { Commands };\n",
    )];
    let main = "import { /*@V,C:i*/Commands } from \"pkg\";\ndeclare module \"pkg\" {\n  const /*@V:d*/Commands: number;\n}\nexport const /*@v:de*/v: number = /*@V:u*/Commands;\nexport declare const /*@c:de*/c: /*@C:u*/Commands<number>;\n";
    let fixture = with_package(&package, main, &[]);
    let project = load(&fixture, "constant beside an interface");
    fixture.check_markers(&project);
    assert_eq!(
        package_declarations(&fixture, &project, "C"),
        ["node_modules/pkg/index.d.ts:1:11"]
    );
}

#[test]
fn resolutions_made_while_merging_see_later_merges() {
    // Finding `Commands` through `barrel`'s star export resolves `mid`'s
    // re-export before the class gives the declaration a value; `use.ts`'s
    // value use through `mid` still finds it, as the oracle shows.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "api.ts",
            "interface /*@C:d*/Commands { /*@!*/base: number }\nexport { /*@C:e*/Commands };\n",
        ),
        ("mid.ts", "export { /*@C:e*/Commands } from \"./api\";\n"),
        ("barrel.ts", "export * from \"./mid\";\n"),
        (
            "aug.ts",
            "export {};\ndeclare module \"./barrel\" {\n  class /*@C:d*/Commands { /*@!*/extra: number }\n}\n",
        ),
        (
            "use.ts",
            "import { /*@C:i*/Commands } from \"./mid\";\nexport const /*@made:de*/made = new /*@C:u*/Commands();\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "star route"));
}

#[test]
fn a_namespace_augmentation_of_a_namespace_export_merges_into_its_module() {
    // `export * as ns` names the module itself, so the augmentation adds
    // exports to `inner` that its own importers see.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "inner.ts",
            "export interface /*@Base:de*/Base { /*@!*/a: number }\n",
        ),
        ("api.ts", "export * as /*@ns:e*/ns from \"./inner\";\n"),
        (
            "aug.ts",
            "export {};\ndeclare module \"./api\" {\n  namespace /*@ns:d*/ns { interface /*@Extra:d*/Extra { /*@!*/b: number } }\n}\n",
        ),
        (
            "use.ts",
            "import { /*@ns:i*/ns } from \"./api\";\nimport type { /*@Extra:i*/Extra } from \"./inner\";\nexport declare const /*@e:de*/e: /*@ns:u*/ns./*@Extra:u*/Extra;\nexport declare const /*@f:de*/f: /*@Extra:u*/Extra;\nexport declare const /*@g:de*/g: /*@ns:u*/ns./*@Base:u*/Base;\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "namespace of a module"));
}

#[test]
fn conflicting_augmentations_fail_where_bindings_are_checked() {
    // tsc reports TS2300 at the augmentation and at the export, whether the
    // export is a declaration or an alias of one.
    let main = "import type { Commands } from \"pkg\";\ndeclare module \"pkg\" {\n  type Commands<T> = { custom: T };\n}\nexport declare const c: Commands<number>;\n";
    for (case, index) in [
        ("direct export", "export interface Commands<T = any> {}\n"),
        (
            "export type specifier",
            "interface Commands<T = any> {}\nexport type { Commands };\n",
        ),
    ] {
        let fixture = with_package(&[("index.d.ts", index)], main, &[]);
        let error = load_error(&fixture, case);
        assert_eq!(error.kind, InputErrorKind::Binding, "{case}: {error}");
        assert!(error.path.unwrap().ends_with("main.ts"), "{case}");
        assert_eq!(error.position, Some(at(3, 8)), "{case}");
        assert!(
            error.message.contains("duplicate declaration"),
            "{case}: {}",
            error.message
        );
    }

    // One way, a class augmentation of a pattern's interface beside a
    // constant: tsc reports TS2451 at both pattern declarations and the
    // class. An alias an augmentation declares beside the interface it
    // merges with: tsc reports TS2440 at the alias.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "ambient.d.ts",
            "declare module \"*.css\" { interface X { a: number } const X: number; }\n",
        ),
        (
            "aug.ts",
            "import \"x.css\";\ndeclare module \"x.css\" { class X { b: number } }\n",
        ),
    ]);
    let error = load_error(&fixture, "pattern beside a constant");
    assert_eq!(error.kind, InputErrorKind::Binding, "{error}");
    assert!(error.message.contains("duplicate declaration"), "{error}");
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        ("api.ts", "export interface X { a: number }\n"),
        (
            "aug.ts",
            "import \"./api\";\nnamespace Other { export interface X { b: number } }\ndeclare module \"./api\" { export import X = Other.X; }\n",
        ),
    ]);
    let error = load_error(&fixture, "alias beside an interface");
    assert_eq!(error.kind, InputErrorKind::Binding, "{error}");
    assert!(error.path.unwrap().ends_with("aug.ts"));
    assert!(error.message.contains("conflicts with a local declaration"));
}

/// TypeScript's `mergeSymbol` keeps one side of an excluded pair: the
/// augmentation where it meets an alias (`return source`), the original
/// where it meets the declaration itself (`return target`). In dependencies
/// it reports nothing.
#[test]
fn conflicting_augmentations_in_dependencies_keep_the_side_typescript_keeps() {
    let config =
        r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true }, "files": ["main.ts"] }"#;
    let plugin = "export declare const plugin: number;\ndeclare module \"pkg\" {\n  type Commands = { p: number };\n}\n";
    let main = "import { /*@plugin:i*/plugin } from \"plugin\";\nimport type { /*@C:i*/Commands } from \"pkg\";\nexport declare const /*@c:de*/c: /*@C:u*/Commands;\nexport const /*@n:de*/n: number = /*@plugin:u*/plugin;\n";
    for (case, index, kept) in [
        (
            "alias",
            "interface Commands { a: number }\nexport type { Commands };\n",
            "node_modules/plugin/index.d.ts:3:8",
        ),
        (
            "direct",
            "export interface Commands { a: number }\n",
            "node_modules/pkg/index.d.ts:1:18",
        ),
    ] {
        let fixture = Fixture::new(&[
            ("tsconfig.json", config),
            ("node_modules/pkg/package.json", MANIFEST),
            ("node_modules/pkg/index.d.ts", index),
            (
                "node_modules/plugin/package.json",
                r#"{ "name": "plugin", "types": "index.d.ts" }"#,
            ),
            ("node_modules/plugin/index.d.ts", plugin),
            ("main.ts", main),
        ]);
        let project = load(&fixture, case);
        fixture.check_markers(&project);
        assert_eq!(
            package_declarations(&fixture, &project, "C"),
            [kept],
            "{case}"
        );
    }
}

#[test]
fn augmenting_a_non_module_export_assignment_fails_where_bindings_are_checked() {
    // tsc reports TS2671 at the augmented name when `export =` names no
    // namespace, module, or enum, TS2649 when an augmentation with values
    // meets a constant beside the namespace, and TS2567 when it meets a const
    // enum, which is no namespace; it merges nothing. A dependency's
    // augmentation binds nothing.
    let function = "declare function f(): void;\nexport = f;\n";
    let constant = "declare const pkg: { a: number };\ndeclare namespace pkg { interface Options {} }\nexport = pkg;\n";
    let enumeration = "declare const enum E { A }\nexport = E;\n";
    let types = "import type * as F from \"pkg\";\ndeclare module \"pkg\" {\n  interface Options { custom: number; }\n}\nexport declare const f: typeof F;\n";
    let values = "import type * as F from \"pkg\";\ndeclare module \"pkg\" {\n  interface Options { custom: number; }\n}\nexport {};\ndeclare module \"pkg\" {\n  const added: number;\n}\nexport declare const f: typeof F;\n";
    let value = "import type * as P from \"pkg\";\ndeclare module \"pkg\" {\n  const added: number;\n}\nexport declare const p: typeof P;\n";
    for (case, index, main, message) in [
        ("export = function", function, types, "non-module entity"),
        ("values beside a constant", constant, values, "with values"),
        (
            "values beside a const enum",
            enumeration,
            value,
            "duplicate declaration",
        ),
    ] {
        let fixture = with_package(&[("index.d.ts", index)], main, &[]);
        let error = load_error(&fixture, case);
        assert_eq!(error.kind, InputErrorKind::Binding, "{case}: {error}");
        assert!(error.path.unwrap().ends_with("main.ts"), "{case}");
        assert_eq!(error.position, Some(at(2, 16)), "{case}");
        assert!(error.message.contains(message), "{case}: {}", error.message);
    }
    // Types alone merge into the namespace beside the constant.
    let main = "import type * as P from \"pkg\";\ndeclare module \"pkg\" {\n  interface /*@O:d*/Options { /*@!*/extra: number }\n}\nexport declare const /*@o:de*/o: P./*@O:u*/Options;\n";
    let fixture = with_package(&[("index.d.ts", constant)], main, &[]);
    let project = load(&fixture, "types beside a constant");
    fixture.check_markers(&project);
    assert_eq!(
        package_declarations(&fixture, &project, "O"),
        ["node_modules/pkg/index.d.ts:2:35"]
    );

    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true }, "files": ["main.ts"] }"#,
        ),
        ("node_modules/pkg/package.json", MANIFEST),
        ("node_modules/pkg/index.d.ts", function),
        (
            "node_modules/plugin/package.json",
            r#"{ "name": "plugin", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/plugin/index.d.ts",
            "export declare const plugin: number;\ndeclare module \"pkg\" { interface Commands<T> { custom: T; } }\n",
        ),
        (
            "main.ts",
            "import { /*@plugin:i*/plugin } from \"plugin\";\nimport type * as /*@F:i*/F from \"pkg\";\n/*@plugin:u*/plugin;\nexport declare const /*@f:de*/f: typeof /*@F:u*/F;\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "dependency"));
}

#[test]
fn pattern_augmentations_merge_one_way_for_their_specifier() {
    // The augmented specifier's module has the pattern's and the
    // augmentation's declarations; another specifier matching the pattern
    // still has only the pattern's (tsc reports TS2339 for `b` there).
    let main = "import type { /*@X:i*/Styles } from \"x.css\";\ndeclare module \"x.css\" {\n  interface /*@X:d*/Styles { /*@!*/b: string }\n}\nexport declare const /*@s:de*/s: /*@X:u*/Styles;\n";
    let other = "import type { /*@Y:i*/Styles } from \"y.css\";\nexport declare const /*@t:de*/t: /*@Y:u*/Styles;\n";
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "types": ["css"] }, "files": ["main.ts", "other.ts"] }"#,
        ),
        (
            "node_modules/@types/css/index.d.ts",
            "declare module \"*.css\" {\n  interface Styles { a: string }\n}\n",
        ),
        ("main.ts", main),
        ("other.ts", other),
    ]);
    let project = load(&fixture, "pattern in a package");
    fixture.check_markers(&project);
    let pattern = ["node_modules/@types/css/index.d.ts:2:13"];
    assert_eq!(package_declarations(&fixture, &project, "X"), pattern);
    assert_eq!(package_declarations(&fixture, &project, "Y"), pattern);

    // Where the pattern is a project file, its own declaration is an
    // occurrence of the unaugmented group only.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "css.d.ts",
            "declare module \"*.css\" {\n  interface Styles { a: string }\n}\n",
        ),
        (
            "main.ts",
            "import type { Styles } from \"x.css\";\ndeclare module \"x.css\" {\n  interface Styles { b: string }\n}\nexport declare const s: Styles;\n",
        ),
        (
            "other.ts",
            "import type { Styles } from \"y.css\";\nexport declare const t: Styles;\n",
        ),
    ]);
    let project = load(&fixture, "pattern in the project");
    expect_groups(
        &fixture,
        &project,
        &["main.ts:1:15", "main.ts:3:13", "main.ts:5:25"],
        &[group(
            &["css.d.ts:2:13", "main.ts:3:13"],
            &["main.ts:1:15", "main.ts:3:13", "main.ts:5:25"],
        )],
    );
    expect_groups(
        &fixture,
        &project,
        &["css.d.ts:2:13", "other.ts:1:15", "other.ts:2:25"],
        &[group(
            &["css.d.ts:2:13"],
            &["css.d.ts:2:13", "other.ts:1:15", "other.ts:2:25"],
        )],
    );
}

#[test]
fn global_augmentations_merge_only_at_the_top_level_of_a_module_or_ambient_module() {
    // tsc reports TS2669 at `global` nested in a module augmentation or a
    // namespace, or at the top level of a script, and does not merge it.
    let cases = [
        (
            "nested in a module augmentation",
            "export {};\ndeclare module \"pkg\" {\n  interface Commands {}\n  global {\n    interface Hidden {}\n  }\n}\n",
            at(4, 3),
        ),
        (
            "nested in a namespace",
            "export namespace N { declare global { interface Hidden {} } }\n",
            at(1, 30),
        ),
        (
            "at the top level of a script",
            "declare global { interface Hidden {} }\n",
            at(1, 9),
        ),
    ];
    for (case, main, position) in cases {
        let package = [("index.d.ts", "export interface Commands {}\n")];
        let fixture = with_package(&package, main, &[]);
        let error = load_error(&fixture, case);
        assert_eq!(error.kind, InputErrorKind::Binding, "{case}: {error}");
        assert!(error.path.unwrap().ends_with("main.ts"), "{case}");
        assert_eq!(error.position, Some(position), "{case}");
    }

    // In a dependency the block's names stay local to it; at the top level of
    // a module or directly in a script's ambient module they are global.
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true }, "files": ["main.ts"] }"#,
        ),
        (
            "node_modules/plugin/package.json",
            r#"{ "name": "plugin", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/plugin/index.d.ts",
            "/// <reference path=\"./ambient.d.ts\" />\nexport declare const plugin: number;\nexport namespace N { declare global { interface Hidden {} } }\ndeclare global { interface FromModule {} }\n",
        ),
        (
            "node_modules/plugin/ambient.d.ts",
            "declare module \"foo\" { global { interface FromAmbient {} } }\n",
        ),
        (
            "main.ts",
            "import { /*@plugin:i*/plugin } from \"plugin\";\n/*@plugin:u*/plugin;\nexport declare const /*@a:de*/a: /*@?*/Hidden;\nexport declare const /*@b:de*/b: /*@FromModule:u*/FromModule;\nexport declare const /*@c:de*/c: /*@FromAmbient:u*/FromAmbient;\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "dependency"));
}

#[test]
fn namespace_augmentations_merge_through_exported_import_aliases() {
    // TypeScript merges an augmentation's namespace with the module's export
    // and, recursively, each declaration in it with what the namespace
    // exports under that name, following an `export import` alias to its
    // target; tsc reports nothing.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "api.ts",
            "export namespace /*@Other:de*/Other { export interface /*@X:de*/X { /*@!*/a: number } }\nexport namespace /*@N:de*/N { export import /*@X:ei*/X = /*@Other:u*/Other./*@X:u*/X; }\n",
        ),
        (
            "aug.ts",
            "import \"./api\";\ndeclare module \"./api\" { namespace /*@N:d*/N { interface /*@X:d*/X { /*@!*/b: number } } }\n",
        ),
        (
            "use.ts",
            "import { /*@N:i*/N, /*@Other:i*/Other } from \"./api\";\nexport let /*@x:de*/x: /*@N:u*/N./*@X:u*/X;\nexport let /*@y:de*/y: /*@Other:u*/Other./*@X:u*/X;\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "nested alias"));

    // The same alias at the top level of the module.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "api.ts",
            "export namespace /*@Other:de*/Other { export interface /*@X:de*/X { /*@!*/a: number } }\nexport import /*@X:ei*/X = /*@Other:u*/Other./*@X:u*/X;\n",
        ),
        (
            "aug.ts",
            "import \"./api\";\ndeclare module \"./api\" { interface /*@X:d*/X { /*@!*/b: number } }\n",
        ),
        (
            "use.ts",
            "import { /*@X:i*/X, /*@Other:i*/Other } from \"./api\";\nexport let /*@x:de*/x: /*@X:u*/X;\nexport let /*@y:de*/y: /*@Other:u*/Other./*@X:u*/X;\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "top-level alias"));

    // A nested class gives the aliased interface a value wherever it is
    // found: in its namespace, through the alias, and in the original module.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "api.ts",
            "export namespace /*@N:de*/N { export interface /*@X:de*/X { /*@!*/a: number } }\nexport import /*@X:ei*/X = /*@N:u*/N./*@X:u*/X;\nexport const /*@made:de*/made = new /*@N:u*/N./*@X:u*/X();\n",
        ),
        (
            "aug.ts",
            "import \"./api\";\ndeclare module \"./api\" { namespace /*@N:d*/N { class /*@X:d*/X { /*@!*/b: number } } }\n",
        ),
        (
            "use.ts",
            "import { /*@X:i*/X, /*@N:i*/N } from \"./api\";\nexport const /*@x:de*/x = new /*@X:u*/X();\nexport const /*@y:de*/y = new /*@N:u*/N./*@X:u*/X();\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "nested class"));
}

#[test]
fn pattern_augmentations_merge_one_way_with_what_a_pattern_alias_names() {
    // TypeScript's one-way merge gives the augmented specifier's `X` the
    // pattern's `export import X = Other.X` beside the augmentation's
    // interface, so its group lists `Other.X`'s declaration; `Other.X`'s own
    // group, which the alias names, does not gain the augmentation.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "ambient.d.ts",
            "declare module \"*.css\" { namespace Other { interface X { a: number } } export import X = Other.X; }\n",
        ),
        (
            "aug.ts",
            "import \"x.css\";\ndeclare module \"x.css\" { interface X { b: number } }\n",
        ),
        ("use.ts", "import { X } from \"x.css\";\nexport let x: X;\n"),
    ]);
    let project = load(&fixture, "pattern alias");
    expect_groups(
        &fixture,
        &project,
        &["aug.ts:2:36", "use.ts:1:10", "use.ts:2:15"],
        &[group(
            &["ambient.d.ts:1:54", "aug.ts:2:36"],
            &["aug.ts:2:36", "use.ts:1:10", "use.ts:2:15"],
        )],
    );
    expect_groups(
        &fixture,
        &project,
        &[
            "ambient.d.ts:1:54",
            "ambient.d.ts:1:86",
            "ambient.d.ts:1:96",
        ],
        &[group(
            &["ambient.d.ts:1:54"],
            &[
                "ambient.d.ts:1:54",
                "ambient.d.ts:1:86",
                "ambient.d.ts:1:96",
            ],
        )],
    );
    expect_groups(
        &fixture,
        &project,
        &["ambient.d.ts:1:36", "ambient.d.ts:1:90"],
        &[group(
            &["ambient.d.ts:1:36"],
            &["ambient.d.ts:1:36", "ambient.d.ts:1:90"],
        )],
    );
}

#[test]
fn one_way_pattern_merges_check_type_parameters_they_keep_apart() {
    // A one-way merge keeps the pattern's type parameters its own, so the
    // merged interface lists the augmentation's and then the pattern's, and
    // each declaration must list a prefix that includes every parameter
    // without a default: tsc reports TS2428 at aug.ts(2,36) otherwise, even
    // for two `X<T>`. A bidirectional merge shares `T` and reports a mismatch
    // at the original, api.ts(1,18), as well.
    let pattern = |pattern: &str, augmentation: &str| {
        Fixture::new(&[
            ("tsconfig.json", support::CONFIG),
            (
                "ambient.d.ts",
                &format!("declare module \"*.css\" {{ interface X{pattern} {{ a: number }} }}\n"),
            ),
            (
                "aug.ts",
                &format!(
                    "import \"x.css\";\ndeclare module \"x.css\" {{ interface X{augmentation} {{ b: number }} }}\n"
                ),
            ),
            (
                "use.ts",
                "import { X } from \"x.css\";\nexport let x: X<number>;\n",
            ),
        ])
    };
    for (declared, augmented) in [
        ("<T>", "<U, V>"),
        ("<T>", "<T>"),
        ("<T>", "<T = number>"),
        ("<T>", ""),
        ("", "<T>"),
    ] {
        let case = format!("pattern X{declared}, augmentation X{augmented}");
        let error = load_error(&pattern(declared, augmented), &case);
        assert_eq!(error.kind, InputErrorKind::Binding, "{case}: {error}");
        assert!(error.path.unwrap().ends_with("aug.ts"), "{case}");
        assert_eq!(error.position, Some(at(2, 36)), "{case}");
        assert!(
            error.message.contains("identical type parameters"),
            "{case}"
        );
    }
    for (declared, augmented) in [("<T = number>", "<T>"), ("", "")] {
        let case = format!("pattern X{declared}, augmentation X{augmented}");
        load(&pattern(declared, augmented), &case);
    }
    // With defaults, the augmentation's `T` lists the pattern's one way.
    let fixture = pattern("<T = number>", "<T = number>");
    let project = load(&fixture, "defaults");
    expect_groups(
        &fixture,
        &project,
        &["aug.ts:2:38"],
        &[group(
            &["ambient.d.ts:1:38", "aug.ts:2:38"],
            &["aug.ts:2:38"],
        )],
    );
    expect_groups(
        &fixture,
        &project,
        &["ambient.d.ts:1:38"],
        &[group(&["ambient.d.ts:1:38"], &["ambient.d.ts:1:38"])],
    );

    // A pattern in a dependency is checked against a project augmentation;
    // a dependency's augmentation is not checked.
    let pattern_package = (
        "node_modules/@types/css/index.d.ts",
        "declare module \"*.css\" { interface X<T> { a: T } }\n",
    );
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "types": ["css"] } }"#,
        ),
        pattern_package,
        (
            "aug.ts",
            "import \"x.css\";\ndeclare module \"x.css\" { interface X<T> { b: T } }\n",
        ),
    ]);
    let error = load_error(&fixture, "pattern in a dependency");
    assert_eq!(error.kind, InputErrorKind::Binding, "{error}");
    assert_eq!(error.position, Some(at(2, 36)));
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "types": ["css"], "skipLibCheck": true } }"#,
        ),
        pattern_package,
        (
            "node_modules/plugin/package.json",
            r#"{ "name": "plugin", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/plugin/index.d.ts",
            "export declare const plugin: number;\ndeclare module \"x.css\" { interface X<U, V> { b: U } }\n",
        ),
        (
            "main.ts",
            "import { /*@plugin:i*/plugin } from \"plugin\";\nexport const /*@p:de*/p = /*@plugin:u*/plugin;\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "both in dependencies"));

    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        ("api.ts", "export interface X<T> { a: T }\n"),
        (
            "aug.ts",
            "import \"./api\";\ndeclare module \"./api\" { interface X<U, V> { b: U; c: V } }\n",
        ),
    ]);
    let error = load_error(&fixture, "direct");
    assert!(error.path.unwrap().ends_with("api.ts"));
    assert_eq!(error.position, Some(at(1, 18)));
}

#[test]
fn modules_directly_inside_a_script_ambient_module_augment_their_package() {
    // TypeScript's `IsModuleAugmentationExternal`: a module declared directly
    // inside an ambient module at the top level of a script augments the
    // module it names, as one at the top level of a module does; one at the
    // top level of a script is an ambient module that the package gives way
    // to.
    let package = [("index.d.ts", "export interface X { a: number }\n")];
    let package = |ambient: &str| {
        let mut files = vec![
            ("tsconfig.json", support::CONFIG),
            ("node_modules/pkg/package.json", MANIFEST),
            ("node_modules/pkg/index.d.ts", package[0].1),
            (
                "use.ts",
                "import { /*@X:i*/X } from \"pkg\";\nexport let /*@x:de*/x: /*@X:u*/X;\n",
            ),
        ];
        files.push(("ambient.d.ts", ambient));
        Fixture::new(&files)
    };
    for (case, ambient, declarations) in [
        (
            "nested in a script's ambient module",
            "declare module \"wrapper\" { module \"pkg\" { interface /*@X:d*/X { /*@!*/b: number } } }\n",
            &["node_modules/pkg/index.d.ts:1:18"][..],
        ),
        (
            "top level of a module",
            "export {};\ndeclare module \"pkg\" { interface /*@X:d*/X { /*@!*/b: number } }\n",
            &["node_modules/pkg/index.d.ts:1:18"],
        ),
        (
            "top level of a script",
            "declare module \"pkg\" { interface /*@X:d*/X { /*@!*/b: number } }\n",
            &[],
        ),
    ] {
        let fixture = package(ambient);
        let project = load(&fixture, case);
        fixture.check_markers(&project);
        assert_eq!(
            package_declarations(&fixture, &project, "X"),
            declarations,
            "{case}"
        );
    }

    // The package's other exports stay visible.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        ("node_modules/pkg/package.json", MANIFEST),
        (
            "node_modules/pkg/index.d.ts",
            "export interface Commands { a: number }\nexport declare const value: number;\n",
        ),
        (
            "types.d.ts",
            "declare module \"wrapper\" {\n  module \"pkg\" {\n    interface /*@Commands:d*/Commands { /*@!*/extra: number }\n  }\n}\n",
        ),
        (
            "main.ts",
            "import { /*@value:i*/value } from \"pkg\";\nimport type { /*@Commands:i*/Commands } from \"pkg\";\nexport declare const /*@c:de*/c: /*@Commands:u*/Commands;\nexport const /*@n:de*/n: number = /*@c:u*/c./*@!*/a + /*@c:u*/c./*@!*/extra + /*@value:u*/value;\n",
        ),
    ]);
    let project = load(&fixture, "package exports");
    fixture.check_markers(&project);
    assert_eq!(
        package_declarations(&fixture, &project, "Commands"),
        ["node_modules/pkg/index.d.ts:1:18"]
    );
    assert_eq!(
        package_declarations(&fixture, &project, "value"),
        ["node_modules/pkg/index.d.ts:2:22"]
    );

    // A relative name there is no augmentation: it merges nowhere, without
    // an error.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "api.ts",
            "export interface /*@X:de*/X { /*@!*/a: number }\n",
        ),
        (
            "ambient.d.ts",
            "declare module \"wrapper\" { module \"./api\" { interface /*@Y:d*/X { /*@!*/b: number } } }\n",
        ),
        (
            "use.ts",
            "import { /*@X:i*/X } from \"./api\";\nexport let /*@x:de*/x: /*@X:u*/X;\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "relative name"));
}

#[test]
fn augmentations_of_missing_modules_fail_only_outside_ambient_contexts() {
    // TypeScript reports TS2664 for a missing augmented module only where the
    // augmentation's parent is not ambient: a module that is not a
    // declaration file. In a declaration file, or inside a script's ambient
    // module, it binds nothing.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "mod.d.ts",
            "export declare const /*@m:de*/m: number;\ndeclare module \"missing-a\" { interface /*@A:d*/A {} }\n",
        ),
        (
            "scr.d.ts",
            "declare module \"w\" { module \"missing-b\" { interface /*@B:d*/B {} } }\n",
        ),
        (
            "main.ts",
            "import { /*@m:i*/m } from \"./mod\";\nexport const /*@n:de*/n = /*@m:u*/m;\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "ambient contexts"));
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "main.ts",
            "export {};\ndeclare module \"missing\" { interface A {} }\n",
        ),
    ]);
    let error = load_error(&fixture, "module file");
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
    assert_eq!(error.position, Some(at(2, 16)));
}

#[test]
fn misplaced_ambient_modules_fail_where_bindings_are_checked() {
    // tsc reports TS2435 at a string-named module nested where it is neither
    // an augmentation nor an ambient module, and TS2436 at a script's ambient
    // module with a relative name. Neither merges anywhere: in a dependency
    // the nested module does not shadow the package.
    let cases = [
        (
            "nested in a script's namespace",
            "ambient.d.ts",
            "declare namespace N { module \"pkg\" { interface X { c: number } } }\n",
            at(1, 30),
        ),
        (
            "nested in a module's namespace",
            "main.ts",
            "export namespace N { declare module \"pkg\" { interface X { c: number } } }\n",
            at(1, 37),
        ),
        (
            "nested in a module augmentation",
            "main.ts",
            "export {};\ndeclare module \"pkg\" { module \"inner\" { interface I {} } }\n",
            at(2, 31),
        ),
        (
            "nested twice in a script's ambient module",
            "ambient.d.ts",
            "declare module \"w\" { module \"v\" { module \"pkg\" { interface X { d: number } } } }\n",
            at(1, 42),
        ),
        (
            "relative at the top level of a script",
            "ambient.d.ts",
            "declare module \"./top\" { interface Y {} }\n",
            at(1, 16),
        ),
    ];
    for (case, file, text, position) in cases {
        let fixture = Fixture::new(&[
            ("tsconfig.json", support::CONFIG),
            ("node_modules/pkg/package.json", MANIFEST),
            (
                "node_modules/pkg/index.d.ts",
                "export interface X { a: number }\n",
            ),
            (file, text),
        ]);
        let error = load_error(&fixture, case);
        assert_eq!(error.kind, InputErrorKind::Binding, "{case}: {error}");
        assert!(error.path.unwrap().ends_with(file), "{case}");
        assert_eq!(error.position, Some(position), "{case}");
    }

    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true }, "files": ["main.ts"] }"#,
        ),
        ("node_modules/pkg/package.json", MANIFEST),
        (
            "node_modules/pkg/index.d.ts",
            "export interface X { a: number }\n",
        ),
        (
            "node_modules/plugin/package.json",
            r#"{ "name": "plugin", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/plugin/index.d.ts",
            "export declare const plugin: number;\nexport declare namespace N { module \"pkg\" { interface X { c: number } } }\n",
        ),
        (
            "main.ts",
            "import { /*@plugin:i*/plugin } from \"plugin\";\nimport type { /*@X:i*/X } from \"pkg\";\nexport declare const /*@x:de*/x: /*@X:u*/X;\nexport const /*@p:de*/p = /*@plugin:u*/plugin;\n",
        ),
    ]);
    let project = load(&fixture, "dependency");
    fixture.check_markers(&project);
    assert_eq!(
        package_declarations(&fixture, &project, "X"),
        ["node_modules/pkg/index.d.ts:1:18"]
    );
}

#[test]
fn augmentations_through_an_alias_of_a_shorthand_module_merge_into_the_module() {
    // The export names the shorthand module itself, and TypeScript merges the
    // augmentation's interface into the module symbol: the export, every
    // import of the module, and its module object then have the interface as
    // their type, as the oracle's split of the merged symbol shows.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        ("shim.d.ts", "declare module \"foo\";\n"),
        ("api.ts", "export { /*@X,foo:e*/X } from \"foo\";\n"),
        (
            "aug.ts",
            "export {};\ndeclare module \"./api\" {\n  interface /*@X:d*/X { /*@!*/y: number }\n}\n",
        ),
        (
            "use.ts",
            "import type { /*@X,foo:i*/X } from \"./api\";\nexport declare const /*@v:de*/v: /*@X:u*/X;\nexport const /*@n:de*/n: number = /*@v:u*/v./*@!*/y;\nimport type { /*@X,foo:i*/Y } from \"foo\";\nimport * as /*@X,foo:i*/foo from \"foo\";\nexport declare const /*@w:de*/w: /*@X:u*/Y;\nexport declare const /*@z:de*/z: /*@X:u*/foo;\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "shorthand"));
}

#[test]
fn augmentations_merge_into_what_earlier_merges_made() {
    // The first augmentation merges a class with `Thing`'s interface and
    // namespace, whose exports move to the merged symbol; the second, of the
    // module whose `export =` names `Thing`, still finds `Options` there.
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true } }"#,
        ),
        ("node_modules/pkg/package.json", MANIFEST),
        (
            "node_modules/pkg/index.d.ts",
            "import { Thing } from \"./lib\";\nexport = Thing;\n",
        ),
        (
            "node_modules/pkg/lib.d.ts",
            "interface Thing { i: number }\ndeclare namespace Thing { interface Options { a: number } }\nexport { Thing };\n",
        ),
        (
            "main.ts",
            "import type { /*@Options:i*/Options } from \"pkg\";\ndeclare module \"pkg/lib\" {\n  class /*@Thing:d*/Thing { /*@!*/c: number }\n}\ndeclare module \"pkg\" {\n  interface /*@Options:d*/Options { /*@!*/b: number }\n}\nexport declare const /*@o:de*/o: /*@Options:u*/Options;\nexport const /*@n:de*/n: number = /*@o:u*/o./*@!*/a + /*@o:u*/o./*@!*/b;\n",
        ),
    ]);
    let project = load(&fixture, "stale table");
    fixture.check_markers(&project);
    assert_eq!(
        package_declarations(&fixture, &project, "Options"),
        ["node_modules/pkg/lib.d.ts:2:37"]
    );
    assert_eq!(
        package_declarations(&fixture, &project, "Thing"),
        [
            "node_modules/pkg/lib.d.ts:1:11",
            "node_modules/pkg/lib.d.ts:2:19"
        ]
    );
}

#[test]
fn augmentations_merge_through_module_objects_and_namespace_values() {
    // `export = m` of a namespace import is augmented through the module it
    // names.
    let package = [
        ("index.d.ts", "import * as m from \"./m\";\nexport = m;\n"),
        ("m.d.ts", "export interface Opts { a: number }\n"),
    ];
    let main = "import type { /*@Opts:i*/Opts } from \"pkg\";\ndeclare module \"pkg\" {\n  interface /*@Opts:d*/Opts { /*@!*/b: number }\n}\nexport declare const /*@o:de*/o: /*@Opts:u*/Opts;\nexport const /*@n:de*/n: number = /*@o:u*/o./*@!*/a + /*@o:u*/o./*@!*/b;\n";
    let fixture = with_package(&package, main, &[]);
    let project = load(&fixture, "module object");
    fixture.check_markers(&project);
    assert_eq!(
        package_declarations(&fixture, &project, "Opts"),
        ["node_modules/pkg/m.d.ts:1:18"]
    );

    // An instantiated namespace beside an aliased interface stays a separate
    // symbol, which gives the interface's name a value wherever it is found,
    // including the original module.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "api.ts",
            "interface /*@Commands:d*/Commands { /*@!*/base: number }\nexport type { /*@Commands,Commands2:e*/Commands };\nexport const /*@x:de*/x: number = /*@Commands2:u*/Commands./*@extra:u*/extra;\n",
        ),
        (
            "aug.ts",
            "export {};\ndeclare module \"./api\" {\n  namespace /*@Commands2:d*/Commands { const /*@extra:d*/extra: number }\n}\n",
        ),
        (
            "use.ts",
            "import { /*@Commands,Commands2:i*/Commands } from \"./api\";\nexport const /*@y:de*/y: number = /*@Commands2:u*/Commands./*@extra:u*/extra;\nexport declare const /*@c:de*/c: /*@Commands:u*/Commands;\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "namespace value"));

    // A pattern whose `export =` names a namespace is augmented through that
    // namespace, which every specifier the pattern matches shares.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "css.d.ts",
            "declare module \"*.css\" {\n  namespace /*@styles:d*/styles { interface /*@Styles:d*/Styles { /*@!*/a: string } }\n  export = /*@styles:e*/styles;\n}\n",
        ),
        (
            "main.ts",
            "import type { /*@Styles:i*/Styles } from \"x.css\";\ndeclare module \"x.css\" {\n  interface /*@Styles:d*/Styles { /*@!*/b: string }\n}\nexport declare const /*@s:de*/s: /*@Styles:u*/Styles;\nexport const /*@t:de*/t: string = /*@s:u*/s./*@!*/a + /*@s:u*/s./*@!*/b;\n",
        ),
        (
            "other.ts",
            "import type { /*@Styles:i*/Styles } from \"y.css\";\nexport declare const /*@u:de*/u: /*@Styles:u*/Styles;\nexport const /*@v:de*/v: string = /*@u:u*/u./*@!*/b;\n",
        ),
    ]);
    fixture.check_markers(&load(&fixture, "pattern export ="));
}

/// The error a query at `file:line:column` fails with.
fn error_at(fixture: &Fixture, project: &Project, place: &str) -> QueryErrorKind {
    let mut parts = place.rsplitn(3, ':');
    let column = parts.next().unwrap().parse().unwrap();
    let line = parts.next().unwrap().parse().unwrap();
    let file = parts.next().unwrap();
    support::query_error(project.find_references(ReferenceQuery::At {
        file: fixture.path(file),
        position: at(line, column),
    }))
    .kind
}

/// A failed load's kind, file, and position, for comparison.
fn failure(error: &InputError) -> (InputErrorKind, String, Option<SourcePosition>) {
    let path = error.path.as_ref().expect("a located error");
    let file = path.file_name().unwrap().to_string_lossy().into_owned();
    (error.kind, file, error.position)
}

#[test]
fn global_augmentations_merge_into_the_global_table_by_the_checker_merge() {
    // TypeScript's `initializeChecker` builds the global table: each script's
    // declarations merge file by file (`mergeGlobalSymbol`), then each
    // `declare global` block merges into the table as a module augmentation
    // does (`mergeModuleAugmentation`, `mergeSymbolTable`), recursively and
    // through an exported import alias to its target. tsc reports nothing.
    let nested = |ambient: &str, config: &str| {
        Fixture::new(&[
            ("tsconfig.json", config),
            (
                ambient,
                "declare namespace Other { interface X { a: number } }\ndeclare namespace N { export import X = Other.X; }\n",
            ),
            (
                "aug.ts",
                "export {};\ndeclare global { namespace N { interface X { b: number } } }\n",
            ),
            ("use.ts", "export let x: N.X;\nexport let y: Other.X;\n"),
        ])
    };
    let fixture = nested("ambient.ts", support::CONFIG);
    let project = load(&fixture, "project script");
    let x = [
        "ambient.ts:1:37",
        "ambient.ts:2:37",
        "ambient.ts:2:47",
        "aug.ts:2:42",
        "use.ts:1:17",
        "use.ts:2:21",
    ];
    expect_groups(
        &fixture,
        &project,
        &x,
        &[group(&["ambient.ts:1:37", "aug.ts:2:42"], &x)],
    );
    let n = ["ambient.ts:2:19", "aug.ts:2:28", "use.ts:1:15"];
    expect_groups(
        &fixture,
        &project,
        &n,
        &[group(&["ambient.ts:2:19", "aug.ts:2:28"], &n)],
    );
    let fixture = nested(
        "node_modules/@types/amb/index.d.ts",
        r#"{ "compilerOptions": { "noLib": true, "types": ["amb"] }, "files": ["aug.ts", "use.ts"] }"#,
    );
    let project = load(&fixture, "dependency script");
    let x = ["aug.ts:2:42", "use.ts:1:17", "use.ts:2:21"];
    expect_groups(
        &fixture,
        &project,
        &x,
        &[group(
            &["aug.ts:2:42", "node_modules/@types/amb/index.d.ts:1:37"],
            &x,
        )],
    );
    let n = ["aug.ts:2:28", "use.ts:1:15"];
    expect_groups(
        &fixture,
        &project,
        &n,
        &[group(
            &["aug.ts:2:28", "node_modules/@types/amb/index.d.ts:2:19"],
            &n,
        )],
    );
}

#[test]
fn umd_globals_join_the_global_table_only_where_no_global_has_their_name() {
    // A UMD global (`export as namespace`) enters the table after the
    // declarations of its file's predecessors and only if none has its name
    // (`initializeChecker`'s first-in-wins). A later script namespace or a
    // `declare global` namespace then merges into the module the alias names,
    // so `Lib` has both files' exports; an earlier script namespace keeps the
    // name and `Lib.f` is unresolved (tsc's TS2339). The tool's group for the
    // alias itself is the merged module: see the `umd-global-merge`
    // exception fixture for TypeScript's split.
    let umd = "export declare function f(): void;\nexport as namespace Lib;\n";
    let augmentation =
        "export {};\ndeclare global {\n  namespace Lib { const extra: number; }\n}\n";
    let script = "declare namespace Lib { const extra: number; }\n";
    let uses = "Lib.f();\nLib.extra;\n";
    let umd_project = |roots: &[&str], files: &[(&str, &str)]| {
        let roots: Vec<String> = roots.iter().map(|root| format!("\"{root}\"")).collect();
        let config = format!(
            r#"{{ "compilerOptions": {{ "noLib": true }}, "files": [{}] }}"#,
            roots.join(", ")
        );
        let mut all = vec![("tsconfig.json", config.as_str())];
        all.extend_from_slice(files);
        Fixture::new(&all)
    };
    let f = group(&["umd.d.ts:1:25"], &["umd.d.ts:1:25", "use.ts:1:5"]);
    for roots in [
        ["umd.d.ts", "aug.ts", "use.ts"],
        ["aug.ts", "use.ts", "umd.d.ts"],
    ] {
        let case = format!("augmentation, {roots:?}");
        let fixture = umd_project(
            &roots,
            &[
                ("umd.d.ts", umd),
                ("aug.ts", augmentation),
                ("use.ts", uses),
            ],
        );
        let project = load(&fixture, &case);
        let lib = ["aug.ts:3:13", "umd.d.ts:2:21", "use.ts:1:1", "use.ts:2:1"];
        expect_groups(
            &fixture,
            &project,
            &lib,
            &[group(&["aug.ts:3:13", "umd.d.ts:1:1"], &lib)],
        );
        expect_groups(
            &fixture,
            &project,
            &["use.ts:1:5"],
            std::slice::from_ref(&f),
        );
        expect_groups(
            &fixture,
            &project,
            &["use.ts:2:5"],
            &[group(&["aug.ts:3:25"], &["aug.ts:3:25", "use.ts:2:5"])],
        );
    }
    let roots = ["umd.d.ts", "script.d.ts", "use.ts"];
    let fixture = umd_project(
        &roots,
        &[("umd.d.ts", umd), ("script.d.ts", script), ("use.ts", uses)],
    );
    let project_after = load(&fixture, "script after the UMD file");
    let lib = [
        "script.d.ts:1:19",
        "umd.d.ts:2:21",
        "use.ts:1:1",
        "use.ts:2:1",
    ];
    expect_groups(
        &fixture,
        &project_after,
        &lib,
        &[group(&["script.d.ts:1:19", "umd.d.ts:1:1"], &lib)],
    );
    expect_groups(&fixture, &project_after, &["use.ts:1:5"], &[f]);

    let roots = ["script.d.ts", "umd.d.ts", "use.ts"];
    let fixture = umd_project(
        &roots,
        &[("umd.d.ts", umd), ("script.d.ts", script), ("use.ts", uses)],
    );
    let project_before = load(&fixture, "script before the UMD file");
    let lib = ["script.d.ts:1:19", "use.ts:1:1", "use.ts:2:1"];
    expect_groups(
        &fixture,
        &project_before,
        &lib,
        &[group(&["script.d.ts:1:19"], &lib)],
    );
    expect_groups(
        &fixture,
        &project_before,
        &["umd.d.ts:2:21"],
        &[group(&["umd.d.ts:1:1"], &["umd.d.ts:2:21"])],
    );
    assert_eq!(
        error_at(&fixture, &project_before, "use.ts:1:5"),
        QueryErrorKind::UnresolvedTarget
    );

    // The UMD file in a dependency, reached by a type reference.
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["aug.ts", "use.ts"] }"#,
        ),
        ("aug.ts", augmentation),
        (
            "node_modules/umd/package.json",
            r#"{ "name": "umd", "types": "index.d.ts" }"#,
        ),
        ("node_modules/umd/index.d.ts", umd),
        (
            "use.ts",
            &format!("/// <reference types=\"umd\" />\n{uses}"),
        ),
    ]);
    let project = load(&fixture, "dependency");
    let lib = ["aug.ts:3:13", "use.ts:2:1", "use.ts:3:1"];
    expect_groups(
        &fixture,
        &project,
        &lib,
        &[group(
            &["aug.ts:3:13", "node_modules/umd/index.d.ts:1:1"],
            &lib,
        )],
    );
    expect_groups(
        &fixture,
        &project,
        &["use.ts:2:5"],
        &[group(
            &["node_modules/umd/index.d.ts:1:25"],
            &["use.ts:2:5"],
        )],
    );
}

#[test]
fn global_conflicts_follow_the_binder_in_one_table_and_the_checker_across_tables() {
    // One `declare global` block's declarations are one table, which the
    // binder rejects at the second `let` (TS2451). Across scripts, and
    // between a script and a global augmentation, which merges after every
    // script, the checker merge rejects the later declaration (TS2451,
    // TS2300); tsc reports both sides.
    for (case, files, place) in [
        (
            "one block",
            &[(
                "a.ts",
                "export {};\ndeclare global { let a: number; let a: string; }\n",
            )][..],
            ("a.ts", at(2, 37)),
        ),
        (
            "two scripts",
            &[("s.ts", "let x = 1;\n"), ("t.ts", "let x = 2;\n")],
            ("t.ts", at(1, 5)),
        ),
        (
            "a script and a global block",
            &[
                ("s.ts", "let x = 1;\n"),
                ("aug.ts", "export {};\ndeclare global { let x: number; }\n"),
            ],
            ("aug.ts", at(2, 22)),
        ),
        (
            "an interface and a type alias",
            &[
                ("s.ts", "interface X { a: number }\n"),
                (
                    "aug.ts",
                    "export {};\ndeclare global { type X = number; }\n",
                ),
            ],
            ("aug.ts", at(2, 23)),
        ),
    ] {
        let error = support::load_error(files);
        assert_eq!(
            failure(&error),
            (InputErrorKind::Binding, place.0.to_string(), Some(place.1)),
            "{case}: {error}"
        );
    }
}

#[test]
fn a_pattern_alias_lists_its_target_without_merging_it_into_the_augmentation() {
    // The one-way merge gives the augmented specifier's `X` the pattern's
    // `export import X = Other.X` as an alias declaration (TypeScript's
    // `Interface | Alias` symbol). `Other.X` is not one of its interface
    // declarations: TS2428 does not compare their type parameters and each
    // `T` keeps its own identity, while the group lists `Other.X`, which
    // queries reach through the alias. tsc reports nothing, with or without
    // skipLibCheck.
    for (case, target, augmentation, arguments) in [
        ("one parameter", "<T>", "<T>", "<number>"),
        ("other names and count", "<T>", "<U, V>", "<number, string>"),
        ("defaults", "<T = number>", "<T = number>", "<number>"),
    ] {
        for skip in [true, false] {
            let case = format!("{case}, skipLibCheck {skip}");
            let config =
                format!(r#"{{ "compilerOptions": {{ "noLib": true, "skipLibCheck": {skip} }} }}"#);
            let ambient = format!(
                "declare module \"*.css\" {{ namespace Other {{ interface X{target} {{ a: number }} }} export import X = Other.X; }}\n"
            );
            let fixture = Fixture::new(&[
                ("tsconfig.json", &config),
                ("ambient.d.ts", &ambient),
                (
                    "aug.ts",
                    &format!(
                        "import \"x.css\";\ndeclare module \"x.css\" {{ interface X{augmentation} {{ b: number }} }}\n"
                    ),
                ),
                (
                    "use.ts",
                    &format!("import {{ X }} from \"x.css\";\nexport let x: X{arguments};\n"),
                ),
            ]);
            let project = load(&fixture, &case);
            let merged = ["aug.ts:2:36", "use.ts:1:10", "use.ts:2:15"];
            expect_groups(
                &fixture,
                &project,
                &merged,
                &[group(&["ambient.d.ts:1:54", "aug.ts:2:36"], &merged)],
            );
            // The alias's own name and `Other.X` name the target alone.
            let alias = format!("ambient.d.ts:1:{}", ambient.find("import X").unwrap() + 8);
            let qualified = format!("ambient.d.ts:1:{}", ambient.rfind(".X").unwrap() + 2);
            let target = ["ambient.d.ts:1:54", alias.as_str(), qualified.as_str()];
            expect_groups(
                &fixture,
                &project,
                &target,
                &[group(&["ambient.d.ts:1:54"], &target)],
            );
            expect_groups(
                &fixture,
                &project,
                &["ambient.d.ts:1:56"],
                &[group(&["ambient.d.ts:1:56"], &["ambient.d.ts:1:56"])],
            );
            expect_groups(
                &fixture,
                &project,
                &["aug.ts:2:38"],
                &[group(&["aug.ts:2:38"], &["aug.ts:2:38"])],
            );
        }
    }
}

#[test]
fn a_pattern_specifier_exposes_its_complete_exports_while_augmentations_merge() {
    // `api` re-exports the pattern's `X` through `x.css`, which `aug.ts` also
    // augments with an unrelated `Y`. Resolving `api`'s export while
    // augmentations merge finds the pattern's `X` through `x.css` in either
    // order and through a star export, so the augmentation of `./api` merges
    // with it: the consumer's group lists the pattern's declaration, as
    // tsc's does. `x.css`'s `Y` extends that specifier alone, and `y.css`
    // keeps the pattern's. Without the `Y` augmentation the result is the
    // same. TypeScript's own groups split `api.ts`'s alias and `y.css`'s
    // imports from the merged declaration (the `pattern-export-merge`
    // exception fixture).
    let ambient =
        "declare module \"*.css\" { interface X { a: number } interface Y { y: number } }\n";
    let x_css = "declare module \"x.css\" { interface Y { b: number } }\n";
    let api_aug = "declare module \"./api\" { interface X { c: number } }\n";
    let other = "import { X, Y } from \"y.css\";\nexport let ox: X;\nexport let oy: Y;\n";
    let uses = "import { X } from \"./api\";\nexport let x: X;\n";
    let named = "export { X } from \"x.css\";\n";
    let star = "export * from \"x.css\";\n";
    for (case, api, augmentations, x, y) in [
        (
            "pattern augmentation first",
            named,
            format!("import \"./api\";\n{x_css}{api_aug}"),
            "aug.ts:3:36",
            Some("aug.ts:2:36"),
        ),
        (
            "pattern augmentation last",
            named,
            format!("import \"./api\";\n{api_aug}{x_css}"),
            "aug.ts:2:36",
            Some("aug.ts:3:36"),
        ),
        (
            "star export",
            star,
            format!("import \"./api\";\n{x_css}{api_aug}"),
            "aug.ts:3:36",
            Some("aug.ts:2:36"),
        ),
        (
            "no pattern augmentation",
            named,
            format!("import \"./api\";\n{api_aug}"),
            "aug.ts:2:36",
            None,
        ),
    ] {
        let fixture = Fixture::new(&[
            ("tsconfig.json", support::CONFIG),
            ("ambient.d.ts", ambient),
            ("api.ts", api),
            ("aug.ts", &augmentations),
            ("use.ts", uses),
            ("other.ts", other),
        ]);
        let project = load(&fixture, case);
        let mut occurrences = vec![
            "ambient.d.ts:1:36",
            x,
            "other.ts:1:10",
            "other.ts:2:16",
            "use.ts:1:10",
            "use.ts:2:15",
        ];
        if api == named {
            occurrences.push("api.ts:1:10");
        }
        occurrences.sort();
        let expected = group(&["ambient.d.ts:1:36", x], &occurrences);
        for place in &occurrences {
            assert_eq!(
                groups_at(&fixture, &project, place),
                std::slice::from_ref(&expected),
                "{case}: query at {place}"
            );
        }
        let pattern_y = ["ambient.d.ts:1:62", "other.ts:1:13", "other.ts:3:16"];
        for place in pattern_y {
            assert_eq!(
                groups_at(&fixture, &project, place),
                [group(&["ambient.d.ts:1:62"], &pattern_y)],
                "{case}: query at {place}"
            );
        }
        if let Some(y) = y {
            assert_eq!(
                groups_at(&fixture, &project, y),
                [group(&["ambient.d.ts:1:62", y], &[y])],
                "{case}"
            );
        }
    }
}

#[test]
fn one_way_pattern_merges_check_the_views_their_declarations_select() {
    // TS2428 compares the declarations of the symbol a checked declaration
    // denotes. A dependency's augmentation of a project pattern selects the
    // specifier's merged view, which no checked declaration selects, so the
    // project loads whatever the parameters are (tsc, with skipLibCheck,
    // reports nothing; without it, TS2428 only in the dependency, whose
    // bindings this tool does not check). A project augmentation of a
    // dependency's pattern, or of a project pattern, selects that view and
    // fails where tsc reports TS2428 at main.ts(2,36).
    let parameters = ["<T>", "<T = number>", "<U, V>"];
    let rejected = |pattern: &str, augmentation: &str| {
        matches!(
            (pattern, augmentation),
            ("<T>", _) | ("<T = number>", "<U, V>")
        )
    };
    for &pattern in &parameters[..2] {
        for augmentation in parameters {
            let ambient =
                format!("declare module \"*.css\" {{ interface X{pattern} {{ a: number }} }}\n");
            let augment = format!(
                "declare module \"x.css\" {{ interface X{augmentation} {{ b: number }} }}\n"
            );
            let plugin = format!("import \"x.css\";\n{augment}");
            let main = format!("import {{ X }} from \"x.css\";\n{augment}");
            let case = format!("pattern X{pattern}, augmentation X{augmentation}");
            for skip in [true, false] {
                let fixture = Fixture::new(&[
                    (
                        "tsconfig.json",
                        &format!(
                            r#"{{ "compilerOptions": {{ "noLib": true, "skipLibCheck": {skip} }}, "files": ["ambient.ts", "main.ts"] }}"#
                        ),
                    ),
                    ("ambient.ts", &ambient),
                    (
                        "main.ts",
                        "import \"plugin\";\nimport { X } from \"x.css\";\n",
                    ),
                    (
                        "node_modules/plugin/package.json",
                        r#"{ "name": "plugin", "types": "index.d.ts" }"#,
                    ),
                    ("node_modules/plugin/index.d.ts", &plugin),
                ]);
                load(
                    &fixture,
                    &format!("{case}: dependency augmentation, skipLibCheck {skip}"),
                );
                let fixture = Fixture::new(&[
                    (
                        "tsconfig.json",
                        &format!(
                            r#"{{ "compilerOptions": {{ "noLib": true, "skipLibCheck": {skip}, "types": ["css"] }}, "files": ["main.ts"] }}"#
                        ),
                    ),
                    ("node_modules/@types/css/index.d.ts", &ambient),
                    ("main.ts", &main),
                ]);
                let context = format!("{case}: dependency pattern, skipLibCheck {skip}");
                match Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error) {
                    Ok(_) => assert!(!rejected(pattern, augmentation), "{context} loaded"),
                    Err(error) => {
                        assert!(rejected(pattern, augmentation), "{context}: {error}");
                        assert_eq!(
                            failure(&error),
                            (InputErrorKind::Binding, "main.ts".into(), Some(at(2, 36))),
                            "{context}: {error}"
                        );
                    }
                }
            }
            // Both in the project, and both in dependencies.
            let fixture = Fixture::new(&[
                ("tsconfig.json", support::CONFIG),
                ("ambient.ts", &ambient),
                ("main.ts", &main),
            ]);
            let context = format!("{case}: both in the project");
            match Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error) {
                Ok(_) => assert!(!rejected(pattern, augmentation), "{context} loaded"),
                Err(error) => {
                    assert!(rejected(pattern, augmentation), "{context}: {error}");
                    assert_eq!(
                        failure(&error),
                        (InputErrorKind::Binding, "main.ts".into(), Some(at(2, 36))),
                        "{context}: {error}"
                    );
                }
            }
            let fixture = Fixture::new(&[
                (
                    "tsconfig.json",
                    r#"{ "compilerOptions": { "noLib": true, "types": ["css"] }, "files": ["main.ts"] }"#,
                ),
                ("node_modules/@types/css/index.d.ts", &ambient),
                (
                    "main.ts",
                    "import \"plugin\";\nimport { X } from \"x.css\";\n",
                ),
                (
                    "node_modules/plugin/package.json",
                    r#"{ "name": "plugin", "types": "index.d.ts" }"#,
                ),
                ("node_modules/plugin/index.d.ts", &plugin),
            ]);
            load(&fixture, &format!("{case}: both in dependencies"));
        }
    }
}

#[test]
fn failed_augmentation_targets_share_one_tolerance() {
    // TypeScript passes no module-not-found message for an augmentation whose
    // parent is ambient (a declaration file or a script's ambient module),
    // which silences both TS2664 (missing) and TS2306 (a script), and
    // `@ts-ignore` or skipLibCheck suppresses either; a tolerated target
    // merges nothing. Elsewhere tsc reports the failure, here an
    // `UnresolvedImport`.
    let script = ("script.ts", "interface X { a: number }\n");
    let uses = ("use.ts", "declare const x: X;\nx.a;\n");
    let declaration = (
        "types.d.ts",
        "export {};\ndeclare module \"./script\" {\n  interface X { b: number }\n}\n",
    );
    let ignored = (
        "aug.ts",
        "export {};\n// @ts-ignore\ndeclare module \"./script\" {\n  interface X { b: number }\n}\n",
    );
    let ignored_missing = (
        "aug.ts",
        "export {};\n// @ts-ignore\ndeclare module \"./missing\" {\n  interface X { b: number }\n}\n",
    );
    for (case, augmentation) in [
        ("declaration file", declaration),
        ("@ts-ignore", ignored),
        ("@ts-ignore, missing module", ignored_missing),
    ] {
        let fixture = support::project(&[augmentation, script, uses]);
        let project = load(&fixture, case);
        expect_groups(
            &fixture,
            &project,
            &["use.ts:1:18"],
            &[group(
                &["script.ts:1:11"],
                &["script.ts:1:11", "use.ts:1:18"],
            )],
        );
    }
    let checked = (
        "aug.ts",
        "export {};\ndeclare module \"./script\" {\n  interface X { b: number }\n}\n",
    );
    let error = support::load_error(&[checked, script, uses]);
    assert_eq!(
        failure(&error),
        (
            InputErrorKind::UnresolvedImport,
            "aug.ts".into(),
            Some(at(2, 16))
        ),
        "{error}"
    );
    // A dependency's declaration file is ambient; its non-declaration module
    // is not, and tsc reports TS2664 there even under skipLibCheck, but a
    // dependency's diagnostics are not reported here (`Diagnostics::reports`).
    let package = |file: &str| {
        Fixture::new(&[
            (
                "tsconfig.json",
                r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true }, "files": ["use.ts"] }"#,
            ),
            ("use.ts", "import \"plugin\";\nexport {};\n"),
            (
                "node_modules/plugin/package.json",
                &format!(r#"{{ "name": "plugin", "types": "{file}" }}"#),
            ),
            (
                &format!("node_modules/plugin/{file}"),
                "export {};\ndeclare module \"scr\" {\n  interface X { b: number }\n}\n",
            ),
            (
                "node_modules/scr/package.json",
                r#"{ "name": "scr", "types": "index.d.ts" }"#,
            ),
            ("node_modules/scr/index.d.ts", "interface S {}\n"),
        ])
    };
    load(&package("index.d.ts"), "dependency declaration file");
    load(&package("index.ts"), "dependency module");
}

#[test]
fn string_named_modules_count_only_in_ambient_contexts() {
    // TypeScript's parser lists a string-named module only when it is
    // declared, in an ambient context, or in a declaration file
    // (`collectModuleReferences`); otherwise the checker reports TS1035
    // ("Only ambient modules can use quoted names") and nothing merges.
    let pkg = ("pkg.ts", "export interface X { a: number }\n");
    let uses = ("use.ts", "import { X } from \"./pkg\";\nexport let x: X;\n");
    let undeclared = "export {};\nmodule \"./pkg\" {\n  interface X { b: number }\n}\n";
    let error = support::load_error(&[("a.ts", undeclared), pkg, uses]);
    assert_eq!(
        failure(&error),
        (InputErrorKind::Binding, "a.ts".into(), Some(at(2, 8))),
        "{error}"
    );
    assert!(error.message.contains("quoted names"), "{error}");
    let error = support::load_error(&[(
        "s.ts",
        "module \"foo\" {\n  export interface X { b: number }\n}\n",
    )]);
    assert_eq!(
        failure(&error),
        (InputErrorKind::Binding, "s.ts".into(), Some(at(1, 8))),
        "{error}"
    );
    // In a declaration file it is an augmentation.
    let fixture = support::project(&[("a.d.ts", undeclared), pkg, uses]);
    let project = load(&fixture, "declaration file");
    let x = ["a.d.ts:3:13", "pkg.ts:1:18", "use.ts:1:10", "use.ts:2:15"];
    expect_groups(
        &fixture,
        &project,
        &x,
        &[group(&["a.d.ts:3:13", "pkg.ts:1:18"], &x)],
    );
    // In a dependency's module, it merges nowhere, without an error.
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true }, "files": ["use.ts"] }"#,
        ),
        (
            "use.ts",
            "import \"dep\";\nimport { X } from \"pkg\";\nexport let x: X;\n",
        ),
        (
            "node_modules/dep/package.json",
            r#"{ "name": "dep", "types": "index.ts" }"#,
        ),
        (
            "node_modules/dep/index.ts",
            "export {};\nmodule \"pkg\" {\n  interface X { b: number }\n}\n",
        ),
        ("node_modules/pkg/package.json", MANIFEST),
        (
            "node_modules/pkg/index.d.ts",
            "export interface X { a: number }\n",
        ),
    ]);
    let project = load(&fixture, "dependency module");
    expect_groups(
        &fixture,
        &project,
        &["use.ts:2:10", "use.ts:3:15"],
        &[group(
            &["node_modules/pkg/index.d.ts:1:18"],
            &["use.ts:2:10", "use.ts:3:15"],
        )],
    );
}

#[test]
fn modules_directly_inside_a_script_global_block_augment_their_package() {
    // `IsModuleAugmentationExternal` counts `declare global` as an ambient
    // module, and the parser collects a script's top-level `declare global`
    // statements as those of its ambient modules: a non-relative module
    // directly inside augments its package, though the block itself is
    // TS2669 and merges nowhere. A relative one merges nowhere, without an
    // error.
    let shim = |body: &str| {
        Fixture::new(&[
            (
                "tsconfig.json",
                r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true, "types": ["shim"], "typeRoots": ["./node_modules"] }, "files": ["use.ts"] }"#,
            ),
            (
                "node_modules/shim/index.d.ts",
                &format!("declare global {{\n  {body}\n}}\n"),
            ),
            (
                "node_modules/shim/x.d.ts",
                "export interface X { a: number }\n",
            ),
            ("node_modules/pkg/package.json", MANIFEST),
            (
                "node_modules/pkg/index.d.ts",
                "export interface X { a: number }\n",
            ),
            (
                "use.ts",
                "import { X } from \"pkg\";\nimport { X as Y } from \"shim/x\";\nexport let x: X;\nexport let y: Y;\n",
            ),
        ])
    };
    let fixture = shim("module \"pkg\" { interface X { b: number } }");
    let project = load(&fixture, "package");
    let x = ["use.ts:1:10", "use.ts:3:15"];
    expect_groups(
        &fixture,
        &project,
        &x,
        &[group(
            &[
                "node_modules/pkg/index.d.ts:1:18",
                "node_modules/shim/index.d.ts:2:28",
            ],
            &x,
        )],
    );
    let fixture = shim("module \"./x\" { interface X { b: number } }");
    let project = load(&fixture, "relative");
    expect_groups(
        &fixture,
        &project,
        &["use.ts:2:15", "use.ts:4:15"],
        &[group(
            &["node_modules/shim/x.d.ts:1:18"],
            &["use.ts:2:10", "use.ts:2:15", "use.ts:4:15"],
        )],
    );
}

#[test]
fn augmentations_merge_in_program_order() {
    // `use.ts` imports `./x`, which imports `dep-a`, then `dep-b`, so
    // TypeScript's program lists `dep-a` before `dep-b` (`collectFiles`
    // lists a file after the files it imports) and merges `dep-a`'s
    // interface first; `dep-b`'s conflicting type alias then gives way to
    // it, a dependency conflict tsc does not report under skipLibCheck.
    let augmentation = |declaration: &str| {
        format!("import \"pkg\";\ndeclare module \"pkg\" {{\n  {declaration}\n}}\n")
    };
    let dep_a = augmentation("interface X { a: number }");
    let dep_b = augmentation("type X = { b: number };");
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true }, "files": ["use.ts"] }"#,
        ),
        ("node_modules/pkg/package.json", MANIFEST),
        (
            "node_modules/pkg/index.d.ts",
            "export declare const base: number;\n",
        ),
        (
            "node_modules/dep-a/package.json",
            r#"{ "name": "dep-a", "types": "index.d.ts" }"#,
        ),
        ("node_modules/dep-a/index.d.ts", &dep_a),
        (
            "node_modules/dep-b/package.json",
            r#"{ "name": "dep-b", "types": "index.d.ts" }"#,
        ),
        ("node_modules/dep-b/index.d.ts", &dep_b),
        ("x.ts", "import \"dep-a\";\nexport {};\n"),
        (
            "use.ts",
            "import \"./x\";\nimport \"dep-b\";\nimport type { X } from \"pkg\";\nexport declare const v: X;\n",
        ),
    ]);
    let project = load(&fixture, "order");
    let x = ["use.ts:3:15", "use.ts:4:25"];
    expect_groups(
        &fixture,
        &project,
        &x,
        &[group(&["node_modules/dep-a/index.d.ts:3:13"], &x)],
    );
}

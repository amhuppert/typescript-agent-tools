//! Project loading: which failures a program input's provenance tolerates,
//! and how loading reports the inputs it cannot use.

mod support;

use support::{Fixture, load_error, project};
use typescript_parser::analysis::InputErrorKind;
use typescript_parser::project::Project;
use typescript_parser::references::ReferenceQuery;
use typescript_parser::source::SourcePosition;

const SKIP_LIB_CHECK: &str = r#"{ "compilerOptions": { "noLib": true, "skipLibCheck": true } }"#;

#[test]
fn unsupported_reference_paths_fail_only_project_files() {
    // TypeScript reports TS6504 for a JavaScript reference without allowJs;
    // in a dependency declaration file, skipLibCheck suppresses it. As with
    // an unsupported module target, a dependency's reference is skipped.
    let package = Fixture::new(&[
        ("tsconfig.json", SKIP_LIB_CHECK),
        (
            "node_modules/pkg/package.json",
            r#"{ "name": "pkg", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/pkg/index.d.ts",
            "/// <reference path=\"./x.js\" />\nexport declare const p: number;\n",
        ),
        ("node_modules/pkg/x.js", "export const x = 1;\n"),
        ("app.ts", "import { /*@p:i*/p } from \"pkg\";\n/*@p:u*/p;\n"),
    ]);
    package.check_markers(&package.load());

    let project = Fixture::new(&[
        ("tsconfig.json", SKIP_LIB_CHECK),
        ("x.js", "export const x = 1;\n"),
        (
            "app.d.ts",
            "/// <reference path=\"./x.js\" />\ndeclare const a: number;\n",
        ),
    ]);
    let error = Project::load(&project.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
    assert!(error.path.unwrap().ends_with("app.d.ts"));
}

/// Make `path` unreadable for the test, restoring access for cleanup.
#[cfg(unix)]
struct Unreadable(std::path::PathBuf);

#[cfg(unix)]
impl Unreadable {
    fn new(path: std::path::PathBuf) -> Self {
        Self::with_mode(path, 0o000)
    }

    /// A directory whose names can be listed but whose entries cannot be
    /// inspected.
    fn unsearchable(path: std::path::PathBuf) -> Self {
        Self::with_mode(path, 0o444)
    }

    fn with_mode(path: std::path::PathBuf, mode: u32) -> Self {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        Self(path)
    }
}

#[cfg(unix)]
impl Drop for Unreadable {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

#[cfg(unix)]
#[test]
fn unreadable_package_metadata_is_an_io_error_not_a_missing_manifest() {
    // Without its manifest, `pkg` would silently resolve to `index.d.ts`
    // instead of the `types` entry the manifest names.
    let fixture = Fixture::new(&[
        ("tsconfig.json", SKIP_LIB_CHECK),
        (
            "node_modules/pkg/package.json",
            r#"{ "name": "pkg", "types": "typed.d.ts" }"#,
        ),
        (
            "node_modules/pkg/typed.d.ts",
            "export declare const p: number;\n",
        ),
        (
            "node_modules/pkg/index.d.ts",
            "export declare const p: number;\n",
        ),
        ("app.ts", "import { p } from \"pkg\";\np;\n"),
    ]);
    let manifest = Unreadable::new(fixture.path("node_modules/pkg/package.json"));
    if std::fs::read(&manifest.0).is_ok() {
        eprintln!(
            "SKIP unreadable_package_metadata_is_an_io_error_not_a_missing_manifest: permissions are not enforced, as for the superuser"
        );
        return; // Permissions are not enforced, as for the superuser.
    }
    let error = Project::load(&fixture.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Io, "{error}");
    assert!(error.path.unwrap().ends_with("package.json"));
}

#[cfg(unix)]
#[test]
fn unreadable_type_roots_are_io_errors() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "types": ["*"], "typeRoots": ["./types"] }, "files": ["app.ts"] }"#,
        ),
        ("types/a/index.d.ts", "declare const a: number;\n"),
        ("app.ts", "export const x = 1;\n"),
    ]);
    let root = Unreadable::new(fixture.path("types"));
    if std::fs::read_dir(&root.0).is_ok() {
        eprintln!(
            "SKIP unreadable_type_roots_are_io_errors: permissions are not enforced, as for the superuser"
        );
        return;
    }
    let error = Project::load(&fixture.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Io, "{error}");
}

#[cfg(unix)]
#[test]
fn an_uninspectable_nearer_package_is_an_io_error_not_a_fallback() {
    // A readable nearer package shadows the outer one, so `outer` is not
    // exported; an uninspectable one must not resolve to the outer package.
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "nodenext" } }"#,
        ),
        ("package.json", r#"{ "type": "module" }"#),
        (
            "node_modules/pkg/package.json",
            r#"{ "name": "pkg", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/pkg/index.d.ts",
            "export declare const outer: number;\n",
        ),
        (
            "sub/node_modules/pkg/package.json",
            r#"{ "name": "pkg", "types": "index.d.ts" }"#,
        ),
        (
            "sub/node_modules/pkg/index.d.ts",
            "export declare const inner: number;\n",
        ),
        (
            "sub/a.ts",
            "import { outer } from \"pkg\";\nexport const z = outer;\n",
        ),
    ]);
    let package = Unreadable::new(fixture.path("sub/node_modules/pkg"));
    if std::fs::read_dir(&package.0).is_ok() {
        eprintln!(
            "SKIP an_uninspectable_nearer_package_is_an_io_error_not_a_fallback: permissions are not enforced, as for the superuser"
        );
        return; // Permissions are not enforced, as for the superuser.
    }
    let error = Project::load(&fixture.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Io, "{error}");
    let path = error.path.unwrap();
    assert!(
        path.starts_with(fixture.path("sub/node_modules/pkg"))
            || path.ends_with("sub/node_modules/pkg/package.json"),
        "{}",
        path.display()
    );
}

#[cfg(unix)]
#[test]
fn uninspectable_directory_entries_are_io_errors() {
    // Each directory lists its names, but the entries cannot be inspected,
    // so neither a root nor a type package may be skipped silently.
    let roots = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true } }"#,
        ),
        ("src/a.ts", "export const a = 1;\n"),
    ]);
    let source = Unreadable::unsearchable(roots.path("src"));
    if std::fs::metadata(source.0.join("a.ts")).is_ok() {
        eprintln!(
            "SKIP uninspectable_directory_entries_are_io_errors: permissions are not enforced, as for the superuser"
        );
        return;
    }
    let error = Project::load(&roots.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Io, "{error}");

    let types = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "types": ["*"], "typeRoots": ["./types"] }, "files": ["app.ts"] }"#,
        ),
        ("types/a/index.d.ts", "declare const a: number;\n"),
        ("app.ts", "export const x = 1;\n"),
    ]);
    let root = Unreadable::unsearchable(types.path("types"));
    if std::fs::metadata(root.0.join("a")).is_ok() {
        eprintln!(
            "SKIP uninspectable_directory_entries_are_io_errors: permissions are not enforced, as for the superuser"
        );
        return;
    }
    let error = Project::load(&types.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Io, "{error}");
}

#[test]
fn reference_directives_follow_every_line_break() {
    // `dep.ts` is not a root, so only the directive brings it into the
    // program; a line comment ends at every ECMAScript line terminator.
    for line_break in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
        let app = format!(
            "// lead{line_break}/// <reference path=\"./dep.ts\" />{line_break}depValue;\n"
        );
        let fixture = Fixture::new(&[
            (
                "tsconfig.json",
                r#"{ "compilerOptions": { "noLib": true }, "files": ["app.ts"] }"#,
            ),
            ("dep.ts", "declare const depValue: number;\n"),
            ("app.ts", &app),
        ]);
        let result = fixture
            .load()
            .find_references(ReferenceQuery::Name {
                name: "depValue".into(),
                file: None,
            })
            .unwrap_or_else(|error| panic!("{line_break:?}: {error}"));
        assert_eq!(result.groups.len(), 1, "{line_break:?}");
        assert_eq!(
            result.groups[0].declarations[0].location.file, "dep.ts",
            "{line_break:?}"
        );
    }
}

#[test]
fn literal_inputs_with_unsupported_extensions_fail_by_where_they_are_written() {
    let listed = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["style.css"] }"#,
        ),
        ("style.css", ""),
    ]);
    let error = Project::load(&listed.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Config, "{error}");
    assert!(error.message.contains("unsupported extension"), "{error}");

    let referenced = Fixture::new(&[
        ("tsconfig.json", SKIP_LIB_CHECK),
        ("style.css", ""),
        (
            "app.ts",
            "/// <reference path=\"./style.css\" />\nexport const a = 1;\n",
        ),
    ]);
    let error = Project::load(&referenced.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
    assert_eq!(
        error.message,
        "Referenced file './style.css' has an unsupported extension"
    );

    // Without `resolveJsonModule`, `.json` is such an extension too (TS6054),
    // whether or not the file exists.
    const JSON_OFF: &str = r#"{ "compilerOptions": { "noLib": true, "resolveJsonModule": false }, "files": ["data.json"] }"#;
    for data in [Some("{}"), None] {
        let mut files = vec![("tsconfig.json", JSON_OFF), ("a.ts", "export {};\n")];
        files.extend(data.map(|data| ("data.json", data)));
        let listed = Fixture::new(&files);
        let error = Project::load(&listed.path("tsconfig.json"))
            .map_err(support::input_error)
            .unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Config, "{error}");
        assert!(error.message.contains("unsupported extension"), "{error}");
    }
    let referenced = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "resolveJsonModule": false } }"#,
        ),
        ("data.json", "{}"),
        (
            "app.ts",
            "/// <reference path=\"./data.json\" />\nexport const a = 1;\n",
        ),
    ]);
    let error = Project::load(&referenced.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(
        (error.kind, error.message.as_str()),
        (
            InputErrorKind::Unsupported,
            "Referenced file './data.json' has an unsupported extension"
        )
    );
}

#[test]
fn a_query_path_that_names_no_file_is_outside_the_project() {
    let fixture = Fixture::new(&[
        ("tsconfig.json", SKIP_LIB_CHECK),
        ("app.ts", "export const x = 1;\n"),
    ]);
    let project = fixture.load();
    let Err(typescript_parser::analysis::AnalysisError::Query(error)) =
        project.find_references(typescript_parser::references::ReferenceQuery::At {
            file: "".into(),
            position: typescript_parser::source::SourcePosition { line: 1, column: 1 },
        })
    else {
        panic!("an empty path names no searched file");
    };
    assert_eq!(
        error.kind,
        typescript_parser::analysis::QueryErrorKind::OutsideProject,
        "{error}"
    );
}

#[test]
fn package_metadata_that_is_not_utf8_is_unparsable_not_unreadable() {
    // TypeScript 7.0.2 ignores such a manifest's fields, as for invalid JSON,
    // and resolves the package's `index.d.ts`; it is not an I/O failure.
    let fixture = Fixture::new(&[
        ("tsconfig.json", SKIP_LIB_CHECK),
        (
            "node_modules/pkg/typed.d.ts",
            "export declare const p: number;\n",
        ),
        (
            "node_modules/pkg/index.d.ts",
            "export declare const p: string;\n",
        ),
        ("app.ts", "import { p } from \"pkg\";\np;\n"),
    ]);
    std::fs::write(
        fixture.path("node_modules/pkg/package.json"),
        b"{ \"name\": \"pkg\", \"author\": \"Jos\xe9\", \"types\": \"typed.d.ts\" }",
    )
    .unwrap();
    let result = fixture
        .load()
        .find_references(typescript_parser::references::ReferenceQuery::Name {
            name: "p".into(),
            file: None,
        })
        .unwrap();
    assert_eq!(
        result.groups[0].declarations[0].location.file,
        "node_modules/pkg/index.d.ts"
    );
}

#[test]
fn jsx_text_that_looks_like_a_comment_is_not_a_directive() {
    // tsc reports TS2307 at 2:9: the JSX text is not a comment.
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "jsx": "preserve" } }"#,
        ),
        (
            "view.tsx",
            "export const a = <div>// @ts-ignore\n{import(\"./missing\")}</div>;\n",
        ),
    ]);
    let error = Project::load(&fixture.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
}

#[cfg(unix)]
#[test]
fn files_in_drive_like_directories_keep_their_spelling_through_a_link() {
    // Native globVisitor.visit appends an entry without applying standalone
    // DOS-root rules. The maintained native-entry-paths matrix also checks
    // literal file selection and real/symlink spellings for these names.
    for directory in ["A:", "z:", "1:", "AA:", "A:child", "é:"] {
        for access in ["real", "link"] {
            for literal in [false, true] {
                let mut config: serde_json::Value = serde_json::from_str(SKIP_LIB_CHECK).unwrap();
                if literal {
                    config["files"] = serde_json::json!([format!("./{directory}/x.ts")]);
                }
                let config = config.to_string();
                let file = format!("real/{directory}/x.ts");
                let fixture = Fixture::new(&[
                    ("real/tsconfig.json", &config),
                    (&file, "export const x = 1;\n"),
                ]);
                std::os::unix::fs::symlink(fixture.path("real"), fixture.path("link")).unwrap();
                let config_path = fixture.path(&format!("{access}/tsconfig.json"));
                let project = Project::load(&config_path).unwrap_or_else(|error| {
                    panic!("{directory}/{access}/literal={literal}: {error}")
                });
                let result = project
                    .find_references(ReferenceQuery::At {
                        file: fixture.path(&format!("{access}/{directory}/x.ts")),
                        position: SourcePosition {
                            line: 1,
                            column: 14,
                        },
                    })
                    .unwrap_or_else(|error| {
                        panic!("{directory}/{access}/literal={literal}: {error}")
                    });
                assert_eq!(result.groups.len(), 1);
                assert_eq!(result.groups[0].declarations.len(), 1);
                assert_eq!(
                    result.groups[0].declarations[0].location.file,
                    format!("{directory}/x.ts"),
                );
            }
        }
    }
}

#[test]
fn output_directories_with_a_trailing_separator_map_back_to_sources() {
    // `#x` names a file in `outDir`, which maps back under `rootDir`.
    for name in ["ex", "éx"] {
        let manifest = format!(r##"{{ "name": "p", "imports": {{ "#x": "./dist/{name}.js" }} }}"##);
        let source = "export const /*@e:de*/e = 1;\n";
        let fixture = Fixture::new(&[
            (
                "tsconfig.json",
                r#"{ "compilerOptions": { "noLib": true, "module": "nodenext", "rootDir": "src", "outDir": "dist/" }, "files": ["src/app.ts"] }"#,
            ),
            ("package.json", &manifest),
            (&format!("src/{name}.ts"), source),
            (
                "src/app.ts",
                "import { /*@e:i*/e } from \"#x\";\n/*@e:u*/e;\n",
            ),
        ]);
        let project = Project::load(&fixture.path("tsconfig.json"))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        fixture.check_markers(&project);
    }
}

#[test]
fn an_include_entry_naming_a_file_selects_nothing_from_it() {
    // A component without `.` names a directory; tsc accepts a file there.
    let fixture = Fixture::new(&[
        (
            "proj/tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["app.ts", "../NOTES"] }"#,
        ),
        ("proj/app.ts", "export const a = 1;\n"),
        ("NOTES", "notes\n"),
    ]);
    let result = fixture
        .load_config("proj/tsconfig.json")
        .find_references(typescript_parser::references::ReferenceQuery::Name {
            name: "a".into(),
            file: None,
        })
        .unwrap();
    assert_eq!(result.groups[0].occurrences[0].location.file, "app.ts");
}

/// TypeScript 7's `processCommentDirective` skips only spaces and tabs
/// after `//` and at the start of a block comment's last line; any other
/// whitespace, ASCII or not, ends the directive. The full matrix, with
/// every line break, is `fixtures/json-probes/suppression-directives.json`.
#[test]
fn comment_directives_follow_only_spaces_and_tabs() {
    for (space, directive) in [
        (" ", true),
        ("\t", true),
        (" \t ", true),
        ("\u{a0}", false),
        ("\u{b}", false),
        ("\u{c}", false),
    ] {
        for comment in [
            format!("//{space}@ts-ignore"),
            format!("/* reason\n{space}* @ts-ignore */"),
        ] {
            let source =
                format!("{comment}\nimport {{ x }} from \"./missing\";\nexport const y = x;\n");
            let fixture = Fixture::new(&[("tsconfig.json", SKIP_LIB_CHECK), ("a.ts", &source)]);
            match Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error) {
                Ok(_) => assert!(directive, "{comment:?} suppressed the error"),
                Err(error) => {
                    assert!(!directive, "{comment:?}: {error}");
                    assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
                }
            }
        }
    }
}

/// docs/references.md, "Compatibility limits": modules and exports are keyed
/// by exact Rust strings, which cannot hold an unpaired UTF-16 surrogate.
/// Replacing one would merge distinct names, so such a name is unsupported
/// where it is a key, and the error is located at its string literal.
#[test]
fn names_with_unpaired_surrogates_are_unsupported_where_they_are_keys() {
    // Without the error, `a` resolved to module "\uD800"'s declaration.
    let error = load_error(&[
        (
            "amb.d.ts",
            "declare module \"\\uD800\" { export const a: number; }\ndeclare module \"\\uD801\" { export const b: number; }\n",
        ),
        (
            "b.ts",
            "import { a } from \"\\uD801\";\nexport const z = a;\n",
        ),
    ]);
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
    assert!(
        error.path.as_ref().unwrap().ends_with("amb.d.ts"),
        "{error}"
    );
    assert_eq!(
        error.position,
        Some(SourcePosition {
            line: 1,
            column: 16
        })
    );
    assert!(error.message.contains(r#""\uD800""#), "{error}");

    let module = "export const a = 1;\n";
    for (source, column) in [
        // Module specifiers of every request kind.
        ("import { a } from \"\\uD801\";\n", 19),
        ("export { a as b } from \"\\uD801\";\n", 24),
        ("import x = require(\"\\uD801\");\n", 20),
        ("export type T = import(\"\\uD801\").X;\n", 24),
        ("export const p = import(\"\\uD801\");\n", 25),
        ("export {};\ndeclare module \"\\uD801\" {}\n", 16),
        // String-literal import and export names.
        ("import { \"\\uD800\" as a } from \"./m\";\n", 10),
        ("export { a as \"\\uD800\" } from \"./m\";\n", 15),
        ("export { \"\\uD800\" as b } from \"./m\";\n", 10),
        ("export * as \"\\uD800\" from \"./m\";\n", 13),
        // An ambient module name.
        (
            "declare module \"\\uD801\" { export const b: number; }\n",
            16,
        ),
    ] {
        let error = load_error(&[("m.ts", module), ("app.ts", source)]);
        assert_eq!(error.kind, InputErrorKind::Unsupported, "{source}: {error}");
        assert!(error.path.as_ref().unwrap().ends_with("app.ts"), "{error}");
        let line = source.lines().count();
        assert_eq!(
            error.position,
            Some(SourcePosition { line, column }),
            "{source}"
        );
    }
}

#[test]
fn valid_surrogate_pairs_and_unreferenced_lone_surrogates_load() {
    // A surrogate pair is an ordinary character. An enum member name is never
    // a key: only identifiers, which cannot hold a lone surrogate, refer to it.
    let fixture = project(&[
        (
            "amb.d.ts",
            "declare module \"\\uD83D\\uDE00\" { export const /*@a:de*/a: number; }\n",
        ),
        (
            "app.ts",
            "import { /*@a:i*/a } from \"\\uD83D\\uDE00\";\nenum /*@E:d*/E { \"\\uD800\" = /*@a:u*/a, /*@!*/b = 2, /*@!*/c = /*@!*/b }\n/*@E:u*/E;\n",
        ),
    ]);
    fixture.check_markers(&fixture.load());
}

/// Whether `app.ts`, a root, brings in `dep.ts` through `directive`: the
/// number of groups for `depValue`, or the load error.
fn reference_outcome(directive: &str) -> Result<usize, InputErrorKind> {
    let app = format!("{directive}\ndepValue;\n");
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["app.ts"] }"#,
        ),
        ("dep.ts", "declare const depValue: number;\n"),
        ("app.ts", &app),
    ]);
    let project = Project::load(&fixture.path("tsconfig.json"))
        .map_err(support::input_error)
        .map_err(|e| e.kind)?;
    let result = project
        .find_references(ReferenceQuery::Name {
            name: "depValue".into(),
            file: None,
        })
        .unwrap();
    Ok(result.groups.len())
}

#[test]
fn reference_directives_separate_their_parts_with_spaces_and_tabs_only() {
    // TypeScript 7.0.2: after `///`, any other whitespace makes the comment
    // no directive; after the tag or around `=`, it ends the attributes, so
    // the directive is invalid (TS1084); before the ignored `/>`, anything
    // goes.
    for space in [
        " ", "\t", "\u{b}", "\u{c}", "\u{a0}", "\u{85}", "\u{feff}", "\u{2002}", "\u{3000}",
    ] {
        let separator = space == " " || space == "\t";
        for (directive, otherwise) in [
            (format!("///{space}<reference path=\"./dep.ts\" />"), Ok(0)),
            (
                format!("/// <reference{space}path=\"./dep.ts\" />"),
                Err(InputErrorKind::Syntax),
            ),
            (
                format!("/// <reference path{space}=\"./dep.ts\" />"),
                Err(InputErrorKind::Syntax),
            ),
            (
                format!("/// <reference path={space}\"./dep.ts\" />"),
                Err(InputErrorKind::Syntax),
            ),
            (format!("/// <reference path=\"./dep.ts\"{space}/>"), Ok(1)),
        ] {
            let expected = if separator { Ok(1) } else { otherwise };
            assert_eq!(reference_outcome(&directive), expected, "{directive:?}");
        }
    }
}

#[test]
fn reference_directives_read_attributes_as_typescript_7_does() {
    for (directive, expected) in [
        // No `/>` is needed, and attributes need no separator.
        ("/// <reference path=\"./dep.ts\">", Ok(1)),
        ("/// <reference path=\"./dep.ts\"", Ok(1)),
        ("/// <reference foo=\"1\"path=\"./dep.ts\" />", Ok(1)),
        // The last of a repeated attribute wins, matched without case.
        (
            "/// <reference path=\"./no.ts\" path=\"./dep.ts\" />",
            Ok(1),
        ),
        (
            "/// <reference PATH=\"./no.ts\" path=\"./dep.ts\" />",
            Ok(1),
        ),
        // A present `types`, even empty, decides the directive, and an
        // empty value names a missing target (TS2688, TS6231).
        (
            "/// <reference types=\"\" path=\"./dep.ts\" />",
            Err(InputErrorKind::UnresolvedImport),
        ),
        (
            "/// <reference path=\"\" />",
            Err(InputErrorKind::UnresolvedImport),
        ),
        // Names are ASCII letters and `-`.
        ("/// <reference- path=\"./dep.ts\" />", Ok(0)),
        ("/// <reference/>", Err(InputErrorKind::Syntax)),
        (
            "/// <reference1 path=\"./dep.ts\" />",
            Err(InputErrorKind::Syntax),
        ),
        (
            "/// <reference pa_th=\"1\" path=\"./dep.ts\" />",
            Err(InputErrorKind::Syntax),
        ),
        (
            "/// <reference foo path=\"./dep.ts\" />",
            Err(InputErrorKind::Syntax),
        ),
    ] {
        assert_eq!(reference_outcome(directive), expected, "{directive:?}");
    }
}

#[test]
fn a_shebang_after_a_byte_order_mark_precedes_leading_directives() {
    // TypeScript removes a byte order mark when it reads a file, so the
    // `#!` line still precedes the leading comments.
    for prefix in ["", "\u{feff}"] {
        let shebang = format!("{prefix}#!/usr/bin/env node");
        assert_eq!(
            reference_outcome(&format!("{shebang}\n/// <reference path=\"./dep.ts\" />")),
            Ok(1),
            "{prefix:?}"
        );
        assert_eq!(
            reference_outcome(&format!("{shebang}\n/// <reference bogus=\"x\" />")),
            Err(InputErrorKind::Syntax),
            "{prefix:?}"
        );
        // A `// @ts-ignore` inside the `#!` line is not a comment, so it
        // suppresses nothing (tsc reports TS2307).
        let error = load_error(&[(
            "app.ts",
            &format!("{shebang} // @ts-ignore\nimport {{ x }} from \"./missing\";\nexport {{}};\n"),
        )]);
        assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{prefix:?}");
    }
}

#[test]
fn jsx_pragmas_separate_their_parts_with_spaces_and_tabs_only() {
    // Only `react/jsx-runtime` exists, so a project that reads the pragma
    // requests `preact/jsx-runtime` and fails to load.
    for (pragma, reads) in [
        ("/* @jsxImportSource preact */", true),
        ("/* @jsxImportSource\tpreact */", true),
        ("/* @jsxImportSource preact*/", true),
        ("/*\u{a0}@jsxImportSource preact */", true),
        ("/* @jsxImportSource\u{a0}preact */", false),
        ("/* @jsxImportSource\u{b}preact */", false),
        ("/* @jsxImportSource: preact */", false),
    ] {
        let fixture = Fixture::new(&[
            (
                "tsconfig.json",
                r#"{ "compilerOptions": { "noLib": true, "jsx": "react-jsx", "moduleResolution": "bundler", "module": "esnext" } }"#,
            ),
            ("node_modules/react/package.json", r#"{ "name": "react" }"#),
            (
                "node_modules/react/jsx-runtime.d.ts",
                "export declare function jsx(...a: any[]): any;\n",
            ),
            (
                "view.tsx",
                &format!("{pragma}\nexport const view = <div />;\n"),
            ),
        ]);
        let loaded = Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error);
        match loaded {
            Ok(_) => assert!(!reads, "{pragma:?} was not read"),
            Err(error) => {
                assert!(reads, "{pragma:?}: {error}");
                assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
                assert!(error.message.contains("preact/jsx-runtime"), "{error}");
            }
        }
    }
}

/// The file `value` in `main.ts` resolves to when `node_modules/pkg` has
/// `manifest`, `real.d.ts`, and `index.d.ts`, or the load error.
fn manifest_outcome(manifest: &str) -> Result<String, typescript_parser::analysis::InputError> {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "module": "esnext", "moduleResolution": "bundler" }, "files": ["main.ts"] }"#,
        ),
        ("node_modules/pkg/package.json", manifest),
        (
            "node_modules/pkg/real.d.ts",
            "export declare const value: number;\n",
        ),
        (
            "node_modules/pkg/index.d.ts",
            "export declare const value: number;\n",
        ),
        ("main.ts", "import { value } from \"pkg\";\nvalue;\n"),
    ]);
    let result = Project::load(&fixture.path("tsconfig.json"))
        .map_err(support::input_error)?
        .find_references(ReferenceQuery::Name {
            name: "value".into(),
            file: None,
        })
        .unwrap();
    Ok(result.groups[0].declarations[0].location.file.clone())
}

#[test]
fn package_manifests_are_strict_json() {
    // TypeScript 7.0.2 reads `package.json` as strict JSON: anything else is
    // unparsable, so `types` is ignored and `index.d.ts` is used.
    let real = "node_modules/pkg/real.d.ts";
    let index = "node_modules/pkg/index.d.ts";
    for (manifest, expected) in [
        (r#"{ "types": "./real.d.ts" }"#, real),
        ("\u{feff}{ \"types\": \"./real.d.ts\" }", real),
        (r#"{ "types": "./no.d.ts", "types": "./real.d.ts" }"#, real),
        (
            r#"{ "x": "a\/b😀", "x2": 1e999, "types": "./real.d.ts" }"#,
            real,
        ),
        (r#"{ "types": "./real.d.ts", }"#, index),
        (r#"{ "x": "\x41", "types": "./real.d.ts" }"#, index),
        (r#"{ "x": "\ud800", "types": "./real.d.ts" }"#, index),
        ("{ \"x\": \"a\tb\", \"types\": \"./real.d.ts\" }", index),
        (r#"{ "x": 01, "types": "./real.d.ts" }"#, index),
        (r#"{ 'types': './real.d.ts' }"#, index),
        ("{\u{a0}\"types\": \"./real.d.ts\" }", index),
    ] {
        assert_eq!(
            manifest_outcome(manifest).unwrap(),
            expected,
            "{manifest:?}"
        );
    }
    // A comment is malformed whatever line break ends it.
    for line_break in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
        let manifest = format!("{{ // c{line_break}\"types\": \"./real.d.ts\" }}");
        assert_eq!(
            manifest_outcome(&manifest).unwrap(),
            index,
            "{line_break:?}"
        );
    }
}

#[test]
fn json_nested_past_the_reader_limit_fails_the_load() {
    // TypeScript has no such limit, so a manifest past it is not unparsable:
    // using `index.d.ts` instead would be a different answer.
    let manifest = |depth: usize| {
        format!(
            "{{ \"x\": {}{}, \"types\": \"./real.d.ts\" }}",
            "[".repeat(depth),
            "]".repeat(depth)
        )
    };
    for depth in [126, 127] {
        assert_eq!(
            manifest_outcome(&manifest(depth)).unwrap(),
            "node_modules/pkg/real.d.ts",
            "{depth}"
        );
    }
    for depth in [128, 129] {
        let error = manifest_outcome(&manifest(depth)).unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Unsupported, "{depth}: {error}");
        assert!(
            error
                .path
                .as_ref()
                .unwrap()
                .ends_with("node_modules/pkg/package.json"),
            "{error}"
        );
    }
    // A configuration and a JSON module share TypeScript's JSON reader and
    // its limit: 128 containers, the object holding `x` included.
    let config = |depth: usize| {
        format!(
            "{{ \"compilerOptions\": {{ \"noLib\": true }}, \"x\": {}{} }}",
            "[".repeat(depth),
            "]".repeat(depth)
        )
    };
    for depth in [126, 127] {
        let fixture = Fixture::new(&[("tsconfig.json", &config(depth)), ("a.ts", "export {};\n")]);
        fixture.load();
    }
    for depth in [128, 200, 10_000] {
        let error = load_error_with_config(&config(depth));
        assert_eq!(error.kind, InputErrorKind::Unsupported, "{depth}: {error}");
        assert!(error.path.as_ref().unwrap().ends_with("tsconfig.json"));
    }
    let module = |depth: usize| {
        Fixture::new(&[
            ("tsconfig.json", support::CONFIG),
            ("a.ts", "import data from \"./data.json\";\ndata;\n"),
            (
                "data.json",
                &format!("{}{}", "[".repeat(depth), "]".repeat(depth)),
            ),
        ])
    };
    for depth in [127, 128] {
        module(depth).load();
    }
    for depth in [129, 10_000] {
        let fixture = module(depth);
        let error = Project::load(&fixture.path("tsconfig.json"))
            .map_err(support::input_error)
            .unwrap_err();
        assert_eq!(error.kind, InputErrorKind::Unsupported, "{depth}: {error}");
        assert!(error.path.as_ref().unwrap().ends_with("data.json"));
    }
}

fn load_error_with_config(config: &str) -> typescript_parser::analysis::InputError {
    let fixture = Fixture::new(&[("tsconfig.json", config), ("a.ts", "export {};\n")]);
    Project::load(&fixture.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err()
}

#[test]
fn tsconfig_line_comments_end_at_every_line_break() {
    // TypeScript reads `tsconfig.json` with its scanner: a `//` comment ends
    // at any ECMAScript line terminator, so the `files` after it stays.
    for line_break in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
        let config = format!(
            "{{ \"compilerOptions\": {{ \"noLib\": true }}, // note{line_break}\"files\": [\"b.ts\"] }}"
        );
        let fixture = Fixture::new(&[
            ("tsconfig.json", &config),
            ("a.ts", "export const value = 1;\n"),
            ("b.ts", "export const other = 1;\n"),
        ]);
        let result = fixture
            .load()
            .find_references(ReferenceQuery::Name {
                name: "value".into(),
                file: None,
            })
            .unwrap();
        assert!(result.groups.is_empty(), "{line_break:?}: a.ts was loaded");
    }
}

#[test]
fn tsconfig_strings_use_javascript_escapes_and_keep_unpaired_surrogates() {
    // `\x61` is `a`. An unpaired surrogate names no file, so a `files` entry
    // holding one is unsupported. An unread top-level tool option is harmless,
    // while an unknown compiler option reports TS5023.
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "toolOption": "\ud800", "files": ["\x61.ts"] }"#,
        ),
        ("a.ts", "export const value = 1;\n"),
    ]);
    let result = fixture
        .load()
        .find_references(ReferenceQuery::Name {
            name: "value".into(),
            file: None,
        })
        .unwrap();
    assert_eq!(result.groups.len(), 1);
    let config = r#"{ "compilerOptions": { "noLib": true, "unknownOption": "\ud800" }, "files": ["\x61.ts"] }"#;
    let error = load_error_with_config(config);
    assert_eq!(
        (error.kind, error.position),
        (
            InputErrorKind::Config,
            Some(support::position(
                config,
                config.find("\"unknownOption\"").unwrap()
            ))
        )
    );
    let error = load_error_with_config(
        r#"{ "compilerOptions": { "noLib": true }, "files": ["\ud800.ts"] }"#,
    );
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
}

/// Load a project whose dependency `pkg` re-exports `x` through a package
/// `imports` chain of `redirects` hops, with `options`.
fn imports_chain_outcome(
    redirects: usize,
    options: &str,
) -> Result<Project, typescript_parser::analysis::InputError> {
    let mut imports: Vec<String> = (0..redirects)
        .map(|i| format!(r##""#a{i}": "#a{}""##, i + 1))
        .collect();
    imports.push(format!(r##""#a{redirects}": "./lib.d.ts""##));
    let manifest = format!(
        "{{ \"name\": \"pkg\", \"types\": \"./index.d.ts\", \"imports\": {{ {} }} }}",
        imports.join(", ")
    );
    let config = format!(
        r#"{{ "compilerOptions": {{ "noLib": true, "module": "nodenext"{options} }}, "files": ["main.ts"] }}"#
    );
    let fixture = Fixture::new(&[
        ("tsconfig.json", &config),
        ("package.json", r#"{ "type": "module" }"#),
        ("node_modules/pkg/package.json", &manifest),
        ("node_modules/pkg/index.d.ts", "export * from \"#a0\";\n"),
        (
            "node_modules/pkg/lib.d.ts",
            "export declare const x: number;\n",
        ),
        ("main.ts", "import { x } from \"pkg\";\nx;\n"),
    ]);
    Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error)
}

#[test]
fn an_imports_chain_past_the_limit_fails_even_in_a_dependency() {
    // Dependencies tolerate unsupported inputs, but a chain longer than this
    // tool follows is a limit, not an input TypeScript rejects.
    for options in ["", r#", "skipLibCheck": true"#] {
        for redirects in [1_023, 1_024] {
            imports_chain_outcome(redirects, options)
                .unwrap_or_else(|error| panic!("{redirects}{options}: {error}"));
        }
        let error = imports_chain_outcome(1_025, options).unwrap_err();
        assert_eq!(
            error.kind,
            InputErrorKind::Unsupported,
            "{options}: {error}"
        );
        assert!(
            error
                .path
                .as_ref()
                .unwrap()
                .ends_with("node_modules/pkg/index.d.ts"),
            "{error}"
        );
    }
}

#[test]
fn a_name_no_file_can_have_is_absent_without_a_lookup() {
    // tsc 7.0.2 resolves the import through the ambient module; a NUL byte
    // cannot name a file, so the node_modules lookup finds nothing rather
    // than failing on the file system's refusal.
    let fixture = project(&[
        ("node_modules/.keep", ""),
        (
            "ambient.d.ts",
            "declare module \"a\\0b\" { export const value: number; }\n",
        ),
        ("main.ts", "import { value } from \"a\\0b\";\nvalue;\n"),
    ]);
    let result = fixture
        .load()
        .find_references(ReferenceQuery::Name {
            name: "value".into(),
            file: None,
        })
        .unwrap();
    assert_eq!(result.groups[0].occurrences.len(), 3);
    let error = load_error(&[(
        "main.ts",
        "/// <reference path=\"./a\0b.ts\" />\nexport {};\n",
    )]);
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
}

#[test]
fn a_missing_javascript_input_is_unsupported_wherever_it_is_named() {
    // tsc 7.0.2 reports TS6504 for both: a JavaScript name is rejected before
    // anything looks for the file.
    let error = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "files": ["app.ts", "missing.js"] }"#,
        ),
        ("app.ts", "export {};\n"),
    ]);
    let error = Project::load(&error.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
    let error = load_error(&[(
        "app.ts",
        "/// <reference path=\"./missing.js\" />\nexport {};\n",
    )]);
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
}

#[cfg(unix)]
#[test]
fn an_included_file_without_a_unicode_name_is_unsupported() {
    use std::os::unix::ffi::OsStrExt;
    // Converting the name lossily would select a file that does not exist,
    // or none; the name cannot be represented exactly, so loading says so.
    let fixture = project(&[("a.ts", "export {};\n")]);
    let name = std::ffi::OsStr::from_bytes(b"b\xff.ts");
    if std::fs::write(fixture.root.join(name), "export {};\n").is_err() {
        eprintln!(
            "SKIP an_included_file_without_a_unicode_name_is_unsupported: the file system requires Unicode names"
        );
        return;
    }
    let error = Project::load(&fixture.path("tsconfig.json"))
        .map_err(support::input_error)
        .unwrap_err();
    assert_eq!(error.kind, InputErrorKind::Unsupported, "{error}");
}

/// Loading reports the failure of the first input in load order, however
/// many inputs follow it and whichever failure is found first: a file whose
/// bytes are not UTF-8 cannot be read (`Io`), and a file that does not parse
/// is a `Syntax` failure.
#[test]
fn the_first_failing_input_in_load_order_is_reported_among_many() {
    let invalid: &[u8] = b"export const broken = \"\xff\";\n";
    for (first, first_kind, second) in [
        ("m010.ts", InputErrorKind::Io, "m290.ts"),
        ("m010.ts", InputErrorKind::Syntax, "m290.ts"),
        ("m289.ts", InputErrorKind::Syntax, "m290.ts"),
        ("m289.ts", InputErrorKind::Io, "m290.ts"),
    ] {
        let mut files = vec![("tsconfig.json".to_owned(), support::CONFIG.to_owned())];
        for index in 0..300 {
            files.push((
                format!("m{index:03}.ts"),
                format!("export const value{index} = {index};\n"),
            ));
        }
        let borrowed: Vec<(&str, &str)> = files
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str()))
            .collect();
        let fixture = Fixture::new(&borrowed);
        let fail = |file: &str, kind: InputErrorKind| match kind {
            InputErrorKind::Io => std::fs::write(fixture.path(file), invalid).unwrap(),
            _ => std::fs::write(fixture.path(file), "export const = ;\n").unwrap(),
        };
        fail(first, first_kind);
        // The later failure is of the other kind, so the kind also shows
        // which one was reported.
        let second_kind = if first_kind == InputErrorKind::Io {
            InputErrorKind::Syntax
        } else {
            InputErrorKind::Io
        };
        fail(second, second_kind);
        let error =
            support::input_error(Project::load(&fixture.path("tsconfig.json")).unwrap_err());
        assert_eq!(error.kind, first_kind, "{first} before {second}: {error:?}");
        let reported = error.path.clone().expect("the failure names its file");
        assert_eq!(
            reported.file_name().and_then(|name| name.to_str()),
            Some(first),
            "{error:?}"
        );
    }
}

/// What loading `config` gives, from another thread: nothing when it does
/// not answer within ten seconds, as a read waiting on a pipe never would.
fn load_promptly(
    config: std::path::PathBuf,
) -> Option<Result<(), typescript_parser::analysis::AnalysisError>> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(Project::load(&config).map(|_| ()));
    });
    receiver
        .recv_timeout(std::time::Duration::from_secs(10))
        .ok()
}

/// A pipe with no writer at `path`.
fn pipe(path: &std::path::Path) {
    let made = std::process::Command::new("mkfifo")
        .arg(path)
        .status()
        .unwrap();
    assert!(made.success());
}

/// What a path where loading reads a file names instead.
#[derive(Debug, Clone, Copy)]
enum NotRegular {
    /// A pipe with no writer, which opening would wait on.
    Pipe,
    /// A symbolic link to such a pipe: loading follows links.
    LinkToPipe,
    /// A symbolic link to a device, which reading would read.
    Device,
    /// A directory, where loading reads the configuration.
    Directory,
}

/// A project's files, the path made something else, what it is made, and
/// the path the failure names.
type Refusal<'a> = (&'a [(&'a str, &'a str)], &'a str, NotRegular, &'a str);

/// A source, a configuration, or a package manifest that is not a regular
/// file is refused before it is opened (ENVELOPE.md, "Accepted inputs"):
/// loading fails promptly with an `Io` failure naming the path and saying
/// it is not a regular file, where opening a pipe would wait for a writer
/// and reading a device would read it. An imported input is named by its
/// real path, as loading names every resolved input; the configuration by
/// the path it is given.
#[test]
fn a_path_that_is_not_a_regular_file_is_refused_without_waiting() {
    let imports = r#"{ "compilerOptions": { "noLib": true }, "files": ["first.ts"] }"#;
    let importer = ("first.ts", "import './input';\n");
    let packaged = r#"{ "compilerOptions": { "module": "nodenext", "noLib": true, "types": [] }, "files": ["sub/a.ts"] }"#;
    let manifest = [
        ("tsconfig.json", packaged),
        ("sub/a.ts", "export const a = 1;\n"),
    ];
    let cases: [Refusal; 7] = [
        (
            &[("tsconfig.json", imports), importer],
            "input.ts",
            NotRegular::Pipe,
            "input.ts",
        ),
        (
            &[("tsconfig.json", imports), importer],
            "input.ts",
            NotRegular::LinkToPipe,
            "pipe",
        ),
        (
            &[("tsconfig.json", imports), importer],
            "input.ts",
            NotRegular::Device,
            "/dev/null",
        ),
        (
            &[("a.ts", "export {};\n")],
            "tsconfig.json",
            NotRegular::Pipe,
            "tsconfig.json",
        ),
        (
            &[("a.ts", "export {};\n")],
            "tsconfig.json",
            NotRegular::LinkToPipe,
            "tsconfig.json",
        ),
        (
            &[("a.ts", "export {};\n")],
            "tsconfig.json",
            NotRegular::Directory,
            "tsconfig.json",
        ),
        (
            &manifest,
            "sub/package.json",
            NotRegular::Pipe,
            "sub/package.json",
        ),
    ];
    for (files, refused, kind, named) in cases {
        let fixture = Fixture::new(files);
        let path = fixture.path(refused);
        match kind {
            NotRegular::Pipe => pipe(&path),
            NotRegular::LinkToPipe => {
                let target = fixture.path("pipe");
                pipe(&target);
                std::os::unix::fs::symlink(&target, &path).unwrap();
            }
            NotRegular::Device => std::os::unix::fs::symlink("/dev/null", &path).unwrap(),
            NotRegular::Directory => std::fs::create_dir(&path).unwrap(),
        }
        let result = load_promptly(fixture.path("tsconfig.json"))
            .unwrap_or_else(|| panic!("loading waits on {refused} ({kind:?})"));
        let error =
            support::input_error(result.expect_err(&format!("{refused} ({kind:?}) is refused")));
        assert_eq!(
            error.kind,
            InputErrorKind::Io,
            "{refused} ({kind:?}): {error}"
        );
        assert_eq!(error.message, "not a regular file", "{refused} ({kind:?})");
        let path = error.path.expect("the failure names the path");
        assert!(
            path.ends_with(named),
            "{refused} ({kind:?}): {}",
            path.display()
        );
    }
}

/// A name written literally, as a `files` entry, a reference directive, or
/// a relative `extends`, selects what TypeScript's `FileExists` finds there,
/// any entry but a directory (`getSourceFileFromReference`,
/// `getExtendsConfigPath`): a pipe or a device there is selected, then
/// refused where loading reads it, rather than reported missing or passed
/// over for another candidate, such as `input.tsx` for `input` or
/// `base.json` for `./base`. A selected input is named by its real path; an
/// extended configuration, as every failure to read one, by the extending
/// configuration, whose message names it (`Cannot read file`).
#[test]
fn a_literal_name_that_is_not_a_regular_file_is_refused_not_passed_over() {
    let listed = r#"{ "compilerOptions": { "noLib": true }, "files": ["input.ts"] }"#;
    let extensionless = r#"{ "compilerOptions": { "noLib": true }, "files": ["input"] }"#;
    let referenced = r#"{ "compilerOptions": { "noLib": true }, "files": ["first.ts"] }"#;
    let reference = ("first.ts", "/// <reference path=\"./input.ts\" />\n");
    let extends = r#"{ "extends": "./base", "files": ["a.ts"] }"#;
    let extends_json = r#"{ "extends": "./base.json", "files": ["a.ts"] }"#;
    let base = r#"{ "compilerOptions": { "noLib": true } }"#;
    let source = ("a.ts", "export {};\n");
    let cases: [Refusal; 6] = [
        (
            &[("tsconfig.json", listed)],
            "input.ts",
            NotRegular::Pipe,
            "input.ts",
        ),
        (
            &[("tsconfig.json", listed)],
            "input.ts",
            NotRegular::Device,
            "/dev/null",
        ),
        (
            &[
                ("tsconfig.json", extensionless),
                ("input.tsx", "export {};\n"),
            ],
            "input.ts",
            NotRegular::Pipe,
            "input.ts",
        ),
        (
            &[("tsconfig.json", referenced), reference],
            "input.ts",
            NotRegular::Pipe,
            "input.ts",
        ),
        (
            &[("tsconfig.json", extends), ("base.json", base), source],
            "base",
            NotRegular::Pipe,
            "tsconfig.json",
        ),
        (
            &[("tsconfig.json", extends_json), source],
            "base.json",
            NotRegular::Pipe,
            "tsconfig.json",
        ),
    ];
    for (files, refused, kind, named) in cases {
        let fixture = Fixture::new(files);
        let path = fixture.path(refused);
        match kind {
            NotRegular::Pipe => pipe(&path),
            NotRegular::Device => std::os::unix::fs::symlink("/dev/null", &path).unwrap(),
            NotRegular::LinkToPipe | NotRegular::Directory => unreachable!("not a case here"),
        }
        let result = load_promptly(fixture.path("tsconfig.json"))
            .unwrap_or_else(|| panic!("loading waits on {refused} ({kind:?})"));
        let error =
            support::input_error(result.expect_err(&format!("{refused} ({kind:?}) is refused")));
        assert_eq!(
            error.kind,
            InputErrorKind::Io,
            "{refused} ({kind:?}): {error}"
        );
        let message = if refused.starts_with("base") {
            let target = std::fs::canonicalize(&fixture.root).unwrap().join(refused);
            format!(
                "Cannot read file '{}': not a regular file",
                target.display()
            )
        } else {
            "not a regular file".to_owned()
        };
        assert_eq!(error.message, message, "{refused} ({kind:?}): {error}");
        let path = error.path.expect("the failure names the path");
        assert!(
            path.ends_with(named),
            "{refused} ({kind:?}): {}",
            path.display()
        );
    }
}

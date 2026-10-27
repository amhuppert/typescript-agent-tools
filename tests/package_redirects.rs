//! Package-id deduplication ("redirects") at the final outcome: whether two
//! installed copies of one package are one declaration identity, which copy
//! the program keeps, and what a request naming the other copy answers. A
//! copy declares a class with a protected member and a unique symbol, whose
//! identity a conditional type shows (`"yes"` when one copy's type is
//! assignable to the other's), or a `which` literal naming the copy.
//! Expected answers are TypeScript 5.6.3's and native 7.0.2's (`tsc
//! --explainFiles --incremental false` on scratch copies, the identity
//! probes as `const check: Same = "yes"` assignments): both compilers
//! deduplicate packages (`findSourceFileWorker`, `collectFiles`), 7.0.2
//! unless `deduplicatePackages` is false.

mod support;

use support::Dialect::{TypeScript7, TypeScript56};
use support::{BASE_OPTIONS, DIALECTS, Fixture};
use typescript_parser::analysis::InputErrorKind;
use typescript_parser::project::Project;
use typescript_parser::types::{TypeAnswer, TypeQuery, TypeRequest};

/// A configuration both dialects read alike, without libraries, with
/// `extra` options and `files` as its roots.
fn configuration(extra: &str, files: &[&str]) -> String {
    let files: Vec<String> = files.iter().map(|file| format!("{file:?}")).collect();
    format!(
        r#"{{"compilerOptions":{{{BASE_OPTIONS}{extra}}},"files":[{}]}}"#,
        files.join(",")
    )
}

/// A package manifest whose entry is `index.d.ts`, with `more` fields.
fn manifest(name: &str, version: &str, more: &str) -> String {
    format!(r#"{{"name":"{name}","version":"{version}","types":"index.d.ts"{more}}}"#)
}

/// A copy of the duplicated package, labelled `label`: a class with a
/// protected member and a unique symbol, whose identities distinguish
/// copies, and `which`, which names the copy.
fn copy(label: &str) -> String {
    format!(
        "export declare class Box {{ protected secret: number; }}\nexport declare const tag: unique symbol;\nexport declare const which: \"{label}\";\n"
    )
}

/// The summary of the type named `name` in `src/main.ts`.
fn summary(project: &Project, fixture: &Fixture, name: &str) -> String {
    let answer = project
        .type_of(TypeRequest {
            query: TypeQuery::Name {
                name: name.to_owned(),
                file: Some(fixture.path("src/main.ts")),
            },
            expand: false,
            depth: 0,
        })
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    let TypeAnswer::Selected(answer) = answer else {
        panic!("{name}: {answer:?}");
    };
    answer.description.summary
}

/// Experiment C: `dup@1.0.0` at the root and under package `a`, which
/// returns the nested copy's types. Both copies have one package id.
fn identity_files(extra: &str) -> Vec<(String, String)> {
    vec![
        ("tsconfig.json".into(), configuration(extra, &["src/main.ts"])),
        (
            "node_modules/dup/package.json".into(),
            manifest("dup", "1.0.0", ""),
        ),
        ("node_modules/dup/index.d.ts".into(), copy("x")),
        (
            "node_modules/a/package.json".into(),
            manifest("a", "1.0.0", ""),
        ),
        (
            "node_modules/a/index.d.ts".into(),
            "import { Box, tag } from \"dup\";\nexport declare const made: Box;\nexport declare const aTag: typeof tag;\n".into(),
        ),
        (
            "node_modules/a/node_modules/dup/package.json".into(),
            manifest("dup", "1.0.0", ""),
        ),
        ("node_modules/a/node_modules/dup/index.d.ts".into(), copy("x")),
        (
            "src/main.ts".into(),
            "import { Box, tag } from \"dup\";\nimport { made, aTag } from \"a\";\nexport type SameBox = typeof made extends Box ? \"yes\" : \"no\";\nexport type SameTag = typeof aTag extends typeof tag ? \"yes\" : \"no\";\n".into(),
        ),
    ]
}

fn borrowed(files: &[(String, String)]) -> Vec<(&str, &str)> {
    files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect()
}

#[test]
fn a_duplicated_package_is_one_identity_in_both_dialects() {
    let files = identity_files("");
    for dialect in DIALECTS {
        let fixture = Fixture::with_dialect(dialect, &borrowed(&files));
        let project = fixture
            .try_load()
            .unwrap_or_else(|error| panic!("{dialect:?}: {error}"));
        assert_eq!(
            summary(&project, &fixture, "SameBox"),
            "\"yes\"",
            "{dialect:?}"
        );
        assert_eq!(
            summary(&project, &fixture, "SameTag"),
            "\"yes\"",
            "{dialect:?}"
        );
    }
}

/// 7.0.2 keeps both copies under `deduplicatePackages: false`, and reports
/// TS2322 for both assignments; 5.6.3 has no such option (TS5023).
#[test]
fn deduplicate_packages_false_keeps_both_copies_under_typescript_7() {
    let files = identity_files(r#","deduplicatePackages":false"#);
    let fixture = Fixture::with_dialect(TypeScript7, &borrowed(&files));
    let project = fixture.load();
    assert_eq!(summary(&project, &fixture, "SameBox"), "\"no\"");
    assert_eq!(summary(&project, &fixture, "SameTag"), "\"no\"");
    let files = identity_files(r#","deduplicatePackages":true"#);
    let fixture = Fixture::with_dialect(TypeScript7, &borrowed(&files));
    let project = fixture.load();
    assert_eq!(summary(&project, &fixture, "SameBox"), "\"yes\"");
}

/// Experiment F: the copies are labelled `root` and `nested`. The file
/// collection meets first, depth first, is the program's: importing `dup`
/// before `a` keeps the root copy, and the other order the nested one,
/// whichever specifier names it.
#[test]
fn the_primary_copy_is_the_one_collection_meets_first() {
    for (first, expected) in [("dup", "root"), ("a", "nested")] {
        let imports = if first == "dup" {
            "import { which as w0 } from \"dup\";\nimport { viaA as w1 } from \"a\";\n"
        } else {
            "import { viaA as w1 } from \"a\";\nimport { which as w0 } from \"dup\";\n"
        };
        let main = format!("{imports}export const v0 = w0;\nexport const v1 = w1;\n");
        let config = configuration("", &["src/main.ts"]);
        let root = copy("root");
        let nested = copy("nested");
        let dup = manifest("dup", "1.0.0", "");
        let a = manifest("a", "1.0.0", "");
        for dialect in DIALECTS {
            let fixture = Fixture::with_dialect(
                dialect,
                &[
                    ("tsconfig.json", &config),
                    ("node_modules/dup/package.json", &dup),
                    ("node_modules/dup/index.d.ts", &root),
                    ("node_modules/a/package.json", &a),
                    (
                        "node_modules/a/index.d.ts",
                        "import { which as w } from \"dup\";\nexport declare const viaA: typeof w;\n",
                    ),
                    ("node_modules/a/node_modules/dup/package.json", &dup),
                    ("node_modules/a/node_modules/dup/index.d.ts", &nested),
                    ("src/main.ts", &main),
                ],
            );
            let project = fixture.load();
            let label = format!("\"{expected}\"");
            for name in ["v0", "v1"] {
                assert_eq!(
                    summary(&project, &fixture, name),
                    label,
                    "{dialect:?}, {first} first, {name}"
                );
            }
        }
    }
}

/// Experiment G: `peer@1.0.0` at the root and under `b`, each with a
/// `react` peer. The package id appends each installed peer's version from
/// the `node_modules` holding the package's real directory, so copies whose
/// peers differ, or where one finds none, stay two identities.
#[test]
fn installed_peer_versions_distinguish_copies() {
    for (nested_react, expected) in [
        (Some("19.0.0"), "\"no\""),
        (Some("18.0.0"), "\"yes\""),
        (None, "\"no\""),
    ] {
        for dialect in DIALECTS {
            let files = peer_files(
                r#","peerDependencies":{"react":"*"}"#,
                r#","peerDependencies":{"react":"*"}"#,
                Some("18.0.0"),
                nested_react,
            );
            let fixture = Fixture::with_dialect(dialect, &borrowed(&files));
            let project = fixture.load();
            assert_eq!(
                summary(&project, &fixture, "SameP"),
                expected,
                "{dialect:?}, nested react {nested_react:?}"
            );
        }
    }
}

/// Copies of `peer@1.0.0` at the root and under `b`, with `more` manifest
/// fields each, and the versions of the `react` and `abc` packages installed
/// beside them.
fn peer_files(
    root_more: &str,
    nested_more: &str,
    root_react: Option<&str>,
    nested_react: Option<&str>,
) -> Vec<(String, String)> {
    let peer = "export declare class P { protected p: number; }\n";
    let mut files = vec![
        ("tsconfig.json".into(), configuration("", &["src/main.ts"])),
        (
            "node_modules/abc/package.json".into(),
            r#"{"name":"abc","version":"1.0.0"}"#.into(),
        ),
        (
            "node_modules/peer/package.json".into(),
            manifest("peer", "1.0.0", root_more),
        ),
        ("node_modules/peer/index.d.ts".into(), peer.into()),
        (
            "node_modules/b/package.json".into(),
            manifest("b", "1.0.0", ""),
        ),
        (
            "node_modules/b/index.d.ts".into(),
            "import { P } from \"peer\";\nexport declare const madeP: P;\n".into(),
        ),
        (
            "node_modules/b/node_modules/peer/package.json".into(),
            manifest("peer", "1.0.0", nested_more),
        ),
        ("node_modules/b/node_modules/peer/index.d.ts".into(), peer.into()),
        (
            "node_modules/b/node_modules/abc/package.json".into(),
            r#"{"name":"abc","version":"1.0.0"}"#.into(),
        ),
        (
            "src/main.ts".into(),
            "import { P } from \"peer\";\nimport { madeP } from \"b\";\nexport type SameP = typeof madeP extends P ? \"yes\" : \"no\";\n".into(),
        ),
    ];
    for (directory, version) in [
        ("node_modules/react", root_react),
        ("node_modules/b/node_modules/react", nested_react),
    ] {
        if let Some(version) = version {
            files.push((
                format!("{directory}/package.json"),
                format!(r#"{{"name":"react","version":"{version}"}}"#),
            ));
        }
    }
    files
}

/// The dialects build the peer suffix differently: 5.6.3 in the order the
/// manifest lists its peers (JavaScript's `for…in`), from any object; 7.0.2
/// in sorted order, and only from a valid string map. A peer manifest
/// without a version contributes `undefined` to 5.6.3's suffix and an empty
/// version to 7.0.2's, the same for both copies.
#[test]
fn the_peer_suffix_follows_each_dialects_rules() {
    let cases = [
        // Peers listed in different orders, the same versions installed.
        (
            r#","peerDependencies":{"react":"*","abc":"*"}"#,
            r#","peerDependencies":{"abc":"*","react":"*"}"#,
            "different order",
            Some("18.0.0"),
            ["\"yes\"", "\"no\""],
        ),
        // A peer range that is not a string: 7.0.2 reads no peers, 5.6.3
        // still finds the different installed versions.
        (
            r#","peerDependencies":{"react":1}"#,
            r#","peerDependencies":{"react":1}"#,
            "non-string range",
            Some("19.0.0"),
            ["\"yes\"", "\"no\""],
        ),
        (
            r#","peerDependencies":{"react":"*","abc":"*"}"#,
            r#","peerDependencies":{"react":"*","abc":"*"}"#,
            "same order",
            Some("18.0.0"),
            ["\"yes\"", "\"yes\""],
        ),
    ];
    for (root_more, nested_more, case, nested_react, expected) in cases {
        for (dialect, expected) in DIALECTS.into_iter().zip(expected) {
            let files = peer_files(root_more, nested_more, Some("18.0.0"), nested_react);
            let fixture = Fixture::with_dialect(dialect, &borrowed(&files));
            let project = fixture.load();
            assert_eq!(
                summary(&project, &fixture, "SameP"),
                expected,
                "{dialect:?}, {case}"
            );
        }
    }
    // Peer manifests without a version.
    for dialect in DIALECTS {
        let mut files = peer_files(
            r#","peerDependencies":{"react":"*"}"#,
            r#","peerDependencies":{"react":"*"}"#,
            None,
            None,
        );
        for directory in ["node_modules/react", "node_modules/b/node_modules/react"] {
            files.push((
                format!("{directory}/package.json"),
                r#"{"name":"react"}"#.into(),
            ));
        }
        let fixture = Fixture::with_dialect(dialect, &borrowed(&files));
        let project = fixture.load();
        assert_eq!(
            summary(&project, &fixture, "SameP"),
            "\"yes\"",
            "{dialect:?}, versionless peers"
        );
    }
}

/// A store layout: each copy's directory is a symbolic link into a store
/// whose `node_modules` holds its `react`. The suffix reads the peers beside
/// the real directory, also under `preserveSymlinks`, never the `react`
/// beside the link.
#[cfg(unix)]
#[test]
fn peers_are_read_beside_the_package_directorys_real_path() {
    for (nested_react, expected) in [("19.0.0", "\"no\""), ("18.0.0", "\"yes\"")] {
        for preserve in ["", r#","preserveSymlinks":true"#] {
            for dialect in DIALECTS {
                let peer_manifest =
                    manifest("peer", "1.0.0", r#","peerDependencies":{"react":"*"}"#);
                let config = configuration(preserve, &["src/main.ts"]);
                let nested = format!(r#"{{"name":"react","version":"{nested_react}"}}"#);
                let peer = "export declare class P { protected p: number; }\n";
                let fixture = Fixture::with_dialect(
                    dialect,
                    &[
                        ("tsconfig.json", &config),
                        (".store/a/node_modules/peer/package.json", &peer_manifest),
                        (".store/a/node_modules/peer/index.d.ts", peer),
                        (
                            ".store/a/node_modules/react/package.json",
                            r#"{"name":"react","version":"18.0.0"}"#,
                        ),
                        (".store/b/node_modules/peer/package.json", &peer_manifest),
                        (".store/b/node_modules/peer/index.d.ts", peer),
                        (".store/b/node_modules/react/package.json", &nested),
                        (
                            "node_modules/react/package.json",
                            r#"{"name":"react","version":"17.0.0"}"#,
                        ),
                        ("node_modules/b/package.json", &manifest("b", "1.0.0", "")),
                        (
                            "node_modules/b/index.d.ts",
                            "import { P } from \"peer\";\nexport declare const madeP: P;\n",
                        ),
                        (
                            "src/main.ts",
                            "import { P } from \"peer\";\nimport { madeP } from \"b\";\nexport type SameP = typeof madeP extends P ? \"yes\" : \"no\";\n",
                        ),
                    ],
                );
                std::os::unix::fs::symlink(
                    "../.store/a/node_modules/peer",
                    fixture.path("node_modules/peer"),
                )
                .unwrap();
                std::fs::create_dir_all(fixture.path("node_modules/b/node_modules")).unwrap();
                std::os::unix::fs::symlink(
                    "../../../.store/b/node_modules/peer",
                    fixture.path("node_modules/b/node_modules/peer"),
                )
                .unwrap();
                let project = fixture.load();
                assert_eq!(
                    summary(&project, &fixture, "SameP"),
                    expected,
                    "{dialect:?}, nested react {nested_react}, options {preserve:?}"
                );
            }
        }
    }
}

/// Which routes give the file they reach a package id: a file found inside
/// `node_modules` by a relative name or a `paths` mapping does in both
/// dialects; a directory found that way, through its manifest, only in
/// 5.6.3 (`loadNodeModuleFromDirectory` attaches none in 7.0.2); a
/// substitution with an extension in neither (`tryLoadModuleUsingPaths`).
/// The root copy is reached only by that route, the nested one by `a`.
#[test]
fn which_routes_carry_a_package_id_follows_each_dialect() {
    let cases: [(&str, &str, &str, [&str; 2]); 5] = [
        (
            "",
            "../node_modules/dup",
            "relative directory",
            ["\"no\"", "\"yes\""],
        ),
        (
            "",
            "../node_modules/dup/index",
            "relative file",
            ["\"yes\"", "\"yes\""],
        ),
        (
            r#","paths":{"mapped":["./node_modules/dup"]}"#,
            "mapped",
            "paths directory",
            ["\"no\"", "\"yes\""],
        ),
        (
            r#","paths":{"mapped":["./node_modules/dup/index"]}"#,
            "mapped",
            "paths file",
            ["\"yes\"", "\"yes\""],
        ),
        (
            r#","paths":{"mapped":["./node_modules/dup/index.d.ts"]}"#,
            "mapped",
            "paths file with its extension",
            ["\"no\"", "\"no\""],
        ),
    ];
    for (extra, specifier, case, expected) in cases {
        let config = configuration(extra, &["src/main.ts"]);
        let main = format!(
            "import {{ Box }} from \"{specifier}\";\nimport {{ made }} from \"a\";\nexport type SameBox = typeof made extends Box ? \"yes\" : \"no\";\n"
        );
        let dup = manifest("dup", "1.0.0", "");
        let a = manifest("a", "1.0.0", "");
        let text = copy("x");
        for (dialect, expected) in DIALECTS.into_iter().zip(expected) {
            let fixture = Fixture::with_dialect(
                dialect,
                &[
                    ("tsconfig.json", &config),
                    ("node_modules/dup/package.json", &dup),
                    ("node_modules/dup/index.d.ts", &text),
                    ("node_modules/a/package.json", &a),
                    (
                        "node_modules/a/index.d.ts",
                        "import { Box } from \"dup\";\nexport declare const made: Box;\n",
                    ),
                    ("node_modules/a/node_modules/dup/package.json", &dup),
                    ("node_modules/a/node_modules/dup/index.d.ts", &text),
                    ("src/main.ts", &main),
                ],
            );
            let project = fixture.load();
            assert_eq!(
                summary(&project, &fixture, "SameBox"),
                expected,
                "{dialect:?}, {case}"
            );
        }
    }
}

/// A `/// <reference types>` found in a type root is a directory route:
/// 5.6.3 gives the root `@types/foo` copy, which the directive reaches
/// first, a package id, so the nested copy `x` imports redirects to it;
/// 7.0.2 gives it none, so `x` keeps its own copy.
#[test]
fn a_type_reference_carries_a_package_id_only_under_5_6() {
    let config = configuration("", &["src/main.ts"]);
    let foo = r#"{"name":"@types/foo","version":"1.0.0"}"#;
    for (dialect, expected) in [(TypeScript7, "\"nested\""), (TypeScript56, "\"root\"")] {
        let fixture = Fixture::with_dialect(
            dialect,
            &[
                ("tsconfig.json", &config),
                ("node_modules/@types/foo/package.json", foo),
                (
                    "node_modules/@types/foo/index.d.ts",
                    "export declare const which: \"root\";\n",
                ),
                ("node_modules/x/package.json", &manifest("x", "1.0.0", "")),
                (
                    "node_modules/x/index.d.ts",
                    "import { which } from \"foo\";\nexport declare const viaX: typeof which;\n",
                ),
                ("node_modules/x/node_modules/@types/foo/package.json", foo),
                (
                    "node_modules/x/node_modules/@types/foo/index.d.ts",
                    "export declare const which: \"nested\";\n",
                ),
                (
                    "src/main.ts",
                    "/// <reference types=\"foo\" />\nimport { viaX } from \"x\";\nexport const v = viaX;\n",
                ),
            ],
        );
        let project = fixture.load();
        assert_eq!(summary(&project, &fixture, "v"), expected, "{dialect:?}");
    }
}

/// Which route's package id a file takes: 5.6.3 decides when it first
/// processes the file, by that route's id, so a nested copy first reached
/// by a reference path has none and stays its own file; 7.0.2 takes the id
/// any route gives it, so the nested copy, met first, is the program's and
/// the root copy redirects to it.
#[test]
fn the_route_that_gives_a_file_its_package_id_follows_each_dialect() {
    let config = configuration("", &["src/refs.ts", "src/main.ts"]);
    let dup = manifest("dup", "1.0.0", "");
    let a = manifest("a", "1.0.0", "");
    let root = copy("root");
    let nested = copy("nested");
    for (dialect, expected) in [(TypeScript7, "\"nested\""), (TypeScript56, "\"root\"")] {
        let fixture = Fixture::with_dialect(
            dialect,
            &[
                ("tsconfig.json", &config),
                ("node_modules/dup/package.json", &dup),
                ("node_modules/dup/index.d.ts", &root),
                ("node_modules/a/package.json", &a),
                (
                    "node_modules/a/index.d.ts",
                    "import { which } from \"dup\";\nexport declare const viaA: typeof which;\n",
                ),
                ("node_modules/a/node_modules/dup/package.json", &dup),
                ("node_modules/a/node_modules/dup/index.d.ts", &nested),
                (
                    "src/refs.ts",
                    "/// <reference path=\"../node_modules/a/node_modules/dup/index.d.ts\" />\nexport {};\n",
                ),
                (
                    "src/main.ts",
                    "import { which } from \"dup\";\nimport { viaA } from \"a\";\nexport const v0 = which;\nexport const v1 = viaA;\n",
                ),
            ],
        );
        let project = fixture.load();
        assert_eq!(summary(&project, &fixture, "v0"), expected, "{dialect:?}");
        assert_eq!(
            summary(&project, &fixture, "v1"),
            "\"nested\"",
            "{dialect:?}"
        );
    }
}

/// The duplicate copy and what only it reaches leave the program, so a
/// failure there is not the program's: a syntax error in a file only the
/// redirected copy imports, and a missing reference in the redirected copy
/// a project file references. Both compilers report each once the nested
/// copy is a different version, and so does loading.
#[test]
fn failures_only_in_a_redirected_copys_closure_are_not_reported() {
    for nested_version in ["1.0.0", "2.0.0"] {
        for dialect in DIALECTS {
            // A syntax error in a file only the nested copy imports.
            let dup = manifest("dup", "1.0.0", "");
            let nested = manifest("dup", nested_version, "");
            let config = configuration("", &["src/main.ts"]);
            let nested_copy = format!("import \"./extra\";\n{}", copy("nested"));
            let fixture = Fixture::with_dialect(
                dialect,
                &[
                    ("tsconfig.json", &config),
                    ("node_modules/dup/package.json", &dup),
                    ("node_modules/dup/index.d.ts", &copy("root")),
                    ("node_modules/a/package.json", &manifest("a", "1.0.0", "")),
                    (
                        "node_modules/a/index.d.ts",
                        "import { Box } from \"dup\";\nexport declare const made: Box;\n",
                    ),
                    ("node_modules/a/node_modules/dup/package.json", &nested),
                    ("node_modules/a/node_modules/dup/index.d.ts", &nested_copy),
                    (
                        "node_modules/a/node_modules/dup/extra.d.ts",
                        "export declare const = ;\n",
                    ),
                    (
                        "src/main.ts",
                        "import { Box } from \"dup\";\nimport { made } from \"a\";\nexport const b = made;\n",
                    ),
                ],
            );
            let result = fixture.try_load();
            if nested_version == "1.0.0" {
                result.unwrap_or_else(|error| panic!("{dialect:?}: {error}"));
            } else {
                let error = result.expect_err("the syntax error is the program's");
                assert_eq!(error.kind, InputErrorKind::Syntax, "{dialect:?}: {error}");
                assert!(
                    error
                        .path
                        .as_ref()
                        .is_some_and(|path| path.ends_with("extra.d.ts")),
                    "{dialect:?}: {error}"
                );
            }

            // A file this tool cannot read (its bytes are not UTF-8), which
            // only the nested copy imports.
            let config = configuration("", &["src/main.ts"]);
            let nested_copy = format!("import \"./bytes\";\n{}", copy("nested"));
            let fixture = Fixture::with_dialect(
                dialect,
                &[
                    ("tsconfig.json", &config),
                    ("node_modules/dup/package.json", &dup),
                    ("node_modules/dup/index.d.ts", &copy("root")),
                    ("node_modules/a/package.json", &manifest("a", "1.0.0", "")),
                    (
                        "node_modules/a/index.d.ts",
                        "import { Box } from \"dup\";\nexport declare const made: Box;\n",
                    ),
                    ("node_modules/a/node_modules/dup/package.json", &nested),
                    ("node_modules/a/node_modules/dup/index.d.ts", &nested_copy),
                    (
                        "src/main.ts",
                        "import { Box } from \"dup\";\nimport { made } from \"a\";\nexport const b = made;\n",
                    ),
                ],
            );
            std::fs::write(
                fixture.path("node_modules/a/node_modules/dup/bytes.d.ts"),
                b"export declare const bytes: \"\xff\";\n",
            )
            .unwrap();
            let result = fixture.try_load();
            if nested_version == "1.0.0" {
                result.unwrap_or_else(|error| panic!("{dialect:?}: {error}"));
            } else {
                let error = result.expect_err("the unreadable file is the program's");
                assert_eq!(error.kind, InputErrorKind::Io, "{dialect:?}: {error}");
            }

            // A missing reference in the nested copy, which a project file
            // references, so that it is searched.
            let config = configuration("", &["src/main.ts", "src/refs.ts"]);
            let nested_copy = format!("/// <reference path=\"./gone.d.ts\" />\n{}", copy("nested"));
            let fixture = Fixture::with_dialect(
                dialect,
                &[
                    ("tsconfig.json", &config),
                    ("node_modules/dup/package.json", &dup),
                    ("node_modules/dup/index.d.ts", &copy("root")),
                    ("node_modules/a/package.json", &manifest("a", "1.0.0", "")),
                    (
                        "node_modules/a/index.d.ts",
                        "import { which } from \"dup\";\nexport declare const viaA: typeof which;\n",
                    ),
                    ("node_modules/a/node_modules/dup/package.json", &nested),
                    ("node_modules/a/node_modules/dup/index.d.ts", &nested_copy),
                    (
                        "src/main.ts",
                        "import { which } from \"dup\";\nimport { viaA } from \"a\";\nexport const v0 = which;\nexport const v1 = viaA;\n",
                    ),
                    (
                        "src/refs.ts",
                        "/// <reference path=\"../node_modules/a/node_modules/dup/index.d.ts\" />\nexport {};\n",
                    ),
                ],
            );
            let result = fixture.try_load();
            if nested_version == "1.0.0" {
                let project = result.unwrap_or_else(|error| panic!("{dialect:?}: {error}"));
                assert_eq!(summary(&project, &fixture, "v1"), "\"root\"", "{dialect:?}");
            } else {
                let error = result.expect_err("the missing reference is the program's");
                assert_eq!(
                    error.kind,
                    InputErrorKind::UnresolvedImport,
                    "{dialect:?}: {error}"
                );
                assert!(error.message.contains("gone.d.ts"), "{dialect:?}: {error}");
            }
        }
    }
}

/// Every request that names a duplicate's file is refused, naming the file
/// the program reads in its place and the package id they share: the
/// duplicate's text is not what the program parsed, so answering from the
/// program's file, or editing it, would silently act on another file. This
/// holds whether the duplicate's own routes are searched (a project file
/// references it) or not, and whatever the primary's are.
#[test]
fn requests_naming_a_duplicate_are_refused_naming_the_program_file() {
    use typescript_parser::analysis::AnalysisError;
    use typescript_parser::editing::{Edit, EditRequest, Operation};
    use typescript_parser::navigation::ShowQuery;
    use typescript_parser::references::ReferenceQuery;
    use typescript_parser::source::SourcePosition;
    let dup = manifest("dup", "1.0.0", "");
    let a = manifest("a", "1.0.0", "");
    let root = copy("root");
    let nested = copy("nested");
    let at = SourcePosition {
        line: 3,
        column: 22,
    };
    // (dialect, roots, the duplicate, the program's file)
    let cases = [
        (
            TypeScript7,
            vec!["src/refs.ts", "src/main.ts"],
            "node_modules/dup/index.d.ts",
            "node_modules/a/node_modules/dup/index.d.ts",
        ),
        (
            TypeScript7,
            vec!["src/main.ts", "src/refs.ts"],
            "node_modules/a/node_modules/dup/index.d.ts",
            "node_modules/dup/index.d.ts",
        ),
        (
            TypeScript56,
            vec!["src/main.ts", "src/refs.ts"],
            "node_modules/a/node_modules/dup/index.d.ts",
            "node_modules/dup/index.d.ts",
        ),
        (
            TypeScript56,
            vec!["src/main.ts"],
            "node_modules/a/node_modules/dup/index.d.ts",
            "node_modules/dup/index.d.ts",
        ),
    ];
    for (dialect, roots, duplicate, primary) in cases {
        let config = configuration("", &roots);
        let fixture = Fixture::with_dialect(
            dialect,
            &[
                ("tsconfig.json", &config),
                ("node_modules/dup/package.json", &dup),
                ("node_modules/dup/index.d.ts", &root),
                ("node_modules/a/package.json", &a),
                (
                    "node_modules/a/index.d.ts",
                    "import { which } from \"dup\";\nexport declare const viaA: typeof which;\n",
                ),
                ("node_modules/a/node_modules/dup/package.json", &dup),
                ("node_modules/a/node_modules/dup/index.d.ts", &nested),
                (
                    "src/refs.ts",
                    "/// <reference path=\"../node_modules/a/node_modules/dup/index.d.ts\" />\nexport {};\n",
                ),
                (
                    "src/main.ts",
                    "import { which } from \"dup\";\nimport { viaA } from \"a\";\nexport const v0 = which;\nexport const v1 = viaA;\n",
                ),
            ],
        );
        let project = fixture.load();
        let context = format!("{dialect:?}, roots {roots:?}, {duplicate}");
        let file = fixture.path(duplicate);
        let refused = |result: Result<(), AnalysisError>, operation: &str| {
            let Err(AnalysisError::Input(error)) = result else {
                panic!("{context}, {operation}: {result:?}");
            };
            assert_eq!(
                error.kind,
                InputErrorKind::Unsupported,
                "{context}, {operation}: {error}"
            );
            assert_eq!(
                error.path.as_deref(),
                Some(file.as_path()),
                "{context}, {operation}"
            );
            for part in [primary, "dup/index.d.ts@1.0.0"] {
                assert!(
                    error.message.contains(part),
                    "{context}, {operation}: {error}"
                );
            }
        };
        refused(project.outline(&file).map(drop), "outline");
        refused(
            project
                .show(ShowQuery::At {
                    file: file.clone(),
                    position: at,
                })
                .map(drop),
            "show at",
        );
        refused(
            project
                .show(ShowQuery::Name {
                    name: "which".into(),
                    file: Some(file.clone()),
                })
                .map(drop),
            "show name",
        );
        refused(
            project
                .find_references(ReferenceQuery::At {
                    file: file.clone(),
                    position: at,
                })
                .map(drop),
            "references",
        );
        refused(
            project
                .type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: file.clone(),
                        position: at,
                    },
                    expand: false,
                    depth: 0,
                })
                .map(drop),
            "type",
        );
        refused(
            project
                .edit(EditRequest {
                    edits: vec![Edit {
                        operation: Operation::ReplaceBody,
                        target: ShowQuery::Name {
                            name: "which".into(),
                            file: Some(file.clone()),
                        },
                        fragment: "{}".into(),
                    }],
                })
                .map(drop),
            "edit",
        );
    }
}

/// A session answers as a fresh direct load after each input of package
/// identity changes: a peer's manifest appears beside the nested copy, the
/// hoisted peer's version changes, the nested copy's version changes, and
/// the program's copy is edited. `SameP` shows whether the copies are one
/// identity; `v1`, the label of the copy `b` imports, which copy the
/// program reads for it.
#[test]
fn a_session_refreshes_package_identity_as_a_fresh_load_does() {
    use typescript_parser::analysis::AnalysisError;
    use typescript_parser::project::{ProjectRequest, ProjectResponse, ProjectSession};
    let peer = |label: &str| {
        format!(
            "export declare class P {{ protected p: number; }}\nexport declare const which: \"{label}\";\n"
        )
    };
    let peer_manifest =
        |version: &str| manifest("peer", version, r#","peerDependencies":{"react":"*"}"#);
    let react = |version: &str| format!(r#"{{"name":"react","version":"{version}"}}"#);
    for dialect in DIALECTS {
        let config = configuration("", &["src/main.ts"]);
        let fixture = Fixture::with_dialect(
            dialect,
            &[
                ("tsconfig.json", &config),
                ("node_modules/react/package.json", &react("18.0.0")),
                ("node_modules/peer/package.json", &peer_manifest("1.0.0")),
                ("node_modules/peer/index.d.ts", &peer("root")),
                ("node_modules/b/package.json", &manifest("b", "1.0.0", "")),
                (
                    "node_modules/b/index.d.ts",
                    "import { P, which } from \"peer\";\nexport declare const madeP: P;\nexport declare const viaB: typeof which;\n",
                ),
                (
                    "node_modules/b/node_modules/peer/package.json",
                    &peer_manifest("1.0.0"),
                ),
                (
                    "node_modules/b/node_modules/peer/index.d.ts",
                    &peer("nested"),
                ),
                (
                    "src/main.ts",
                    "import { P, which } from \"peer\";\nimport { madeP, viaB } from \"b\";\nexport type SameP = typeof madeP extends P ? \"yes\" : \"no\";\nexport const v0 = which;\nexport const v1 = viaB;\n",
                ),
            ],
        );
        let path = fixture.path("tsconfig.json");
        let request = |name: &str| {
            ProjectRequest::Type(TypeRequest::new(TypeQuery::Name {
                name: name.into(),
                file: Some(fixture.path("src/main.ts")),
            }))
        };
        let summary = |response: Result<ProjectResponse, AnalysisError>| match response {
            Ok(ProjectResponse::Type(answer)) => match *answer {
                TypeAnswer::Selected(result) => result.description.summary,
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        };
        let mut session = ProjectSession::new(&path).unwrap();
        let mut ask = |step: &str, expected: [&str; 3]| {
            for (name, expected) in ["SameP", "v0", "v1"].into_iter().zip(expected) {
                let refreshed = summary(session.execute(request(name)));
                let fresh = summary(
                    Project::load(&path).and_then(|project| project.execute(request(name))),
                );
                assert_eq!(refreshed, fresh, "{dialect:?}, {step}: {name}");
                assert_eq!(refreshed, expected, "{dialect:?}, {step}: {name}");
            }
        };
        let write = |file: &str, text: &str| {
            let file = fixture.path(file);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        // No `react` beside the nested copy: its id has no peer suffix.
        ask("loaded", ["\"no\"", "\"root\"", "\"nested\""]);
        write(
            "node_modules/b/node_modules/react/package.json",
            &react("18.0.0"),
        );
        ask("peer manifest added", ["\"yes\"", "\"root\"", "\"root\""]);
        write("node_modules/react/package.json", &react("19.0.0"));
        ask("hoisted peer changed", ["\"no\"", "\"root\"", "\"nested\""]);
        write("node_modules/react/package.json", &react("18.0.0"));
        ask("hoisted peer restored", ["\"yes\"", "\"root\"", "\"root\""]);
        write(
            "node_modules/b/node_modules/peer/package.json",
            &peer_manifest("1.0.1"),
        );
        ask(
            "nested version bumped",
            ["\"no\"", "\"root\"", "\"nested\""],
        );
        write(
            "node_modules/b/node_modules/peer/package.json",
            &peer_manifest("1.0.0"),
        );
        ask(
            "nested version restored",
            ["\"yes\"", "\"root\"", "\"root\""],
        );
        write("node_modules/peer/index.d.ts", &peer("edited"));
        ask(
            "program's copy edited",
            ["\"yes\"", "\"edited\"", "\"edited\""],
        );
        write(
            "node_modules/b/node_modules/peer/index.d.ts",
            &peer("renamed"),
        );
        ask(
            "redirected copy edited",
            ["\"yes\"", "\"edited\"", "\"edited\""],
        );
    }
}

/// References across a redirect: `Box` imported from `dup` in one project
/// file and through `a`, whose own copy redirects to the root copy, in
/// another is one symbol, declared in the program's copy, as 5.6.3's
/// `findReferences` and 7.0.2's checker (`tools/semantic-oracle.mjs`) group
/// it; without deduplication 7.0.2 has a symbol per copy.
#[test]
fn references_reach_through_a_redirect() {
    use typescript_parser::references::{ReferenceGroup, ReferenceQuery};
    use typescript_parser::source::SourcePosition;
    let render = |group: &ReferenceGroup| {
        let at = |location: &typescript_parser::references::Location| {
            format!(
                "{}:{}:{}",
                location.file, location.start.line, location.start.column
            )
        };
        let declarations: Vec<String> = group
            .declarations
            .iter()
            .map(|declaration| at(&declaration.location))
            .collect();
        let occurrences: Vec<String> = group
            .occurrences
            .iter()
            .map(|occurrence| at(&occurrence.location))
            .collect();
        (declarations, occurrences)
    };
    let dup = manifest("dup", "1.0.0", "");
    let text = "export declare class Box { protected secret: number; }\n";
    for (dialect, extra, expected) in [
        (
            TypeScript7,
            "",
            (
                vec!["node_modules/dup/index.d.ts:1:22"],
                vec![
                    "src/main.ts:1:10",
                    "src/main.ts:2:25",
                    "src/other.ts:1:10",
                    "src/other.ts:2:25",
                ],
            ),
        ),
        (
            TypeScript56,
            "",
            (
                vec!["node_modules/dup/index.d.ts:1:22"],
                vec![
                    "src/main.ts:1:10",
                    "src/main.ts:2:25",
                    "src/other.ts:1:10",
                    "src/other.ts:2:25",
                ],
            ),
        ),
        (
            TypeScript7,
            r#","deduplicatePackages":false"#,
            (
                vec!["node_modules/dup/index.d.ts:1:22"],
                vec!["src/main.ts:1:10", "src/main.ts:2:25"],
            ),
        ),
    ] {
        let config = configuration(extra, &["src/main.ts", "src/other.ts"]);
        let fixture = Fixture::with_dialect(
            dialect,
            &[
                ("tsconfig.json", &config),
                ("node_modules/dup/package.json", &dup),
                ("node_modules/dup/index.d.ts", text),
                ("node_modules/a/package.json", &manifest("a", "1.0.0", "")),
                (
                    "node_modules/a/index.d.ts",
                    "export { Box } from \"dup\";\n",
                ),
                ("node_modules/a/node_modules/dup/package.json", &dup),
                ("node_modules/a/node_modules/dup/index.d.ts", text),
                (
                    "src/main.ts",
                    "import { Box } from \"dup\";\nexport declare const b: Box;\n",
                ),
                (
                    "src/other.ts",
                    "import { Box } from \"a\";\nexport declare const c: Box;\n",
                ),
            ],
        );
        let project = fixture.load();
        let result = project
            .find_references(ReferenceQuery::At {
                file: fixture.path("src/main.ts"),
                position: SourcePosition {
                    line: 1,
                    column: 10,
                },
            })
            .unwrap_or_else(|error| panic!("{dialect:?}{extra}: {error}"));
        let groups: Vec<_> = result.groups.iter().map(render).collect();
        let expected = (
            expected
                .0
                .iter()
                .map(|s| (*s).to_owned())
                .collect::<Vec<_>>(),
            expected
                .1
                .iter()
                .map(|s| (*s).to_owned())
                .collect::<Vec<_>>(),
        );
        assert_eq!(groups, [expected], "{dialect:?}{extra}");
    }
}

/// Subpath routes into a package: an `exports` subpath gives its file the
/// package's id, so the copies of `dup/sub` are one identity; a
/// `typesVersions` substitution written with its extension gives none
/// (`tryLoadModuleUsingPaths` returns its file without an id), so they stay
/// two, in both compilers.
#[test]
fn package_subpath_routes_follow_both_compilers() {
    let text = "export declare class Box { protected secret: number; }\n";
    for (manifest_text, case, expected) in [
        (
            r#"{"name":"dup","version":"1.0.0","exports":{"./sub":{"types":"./lib/sub.d.ts"}}}"#,
            "exports",
            "\"yes\"",
        ),
        (
            r#"{"name":"dup","version":"1.0.0","typesVersions":{"*":{"sub":["lib/sub.d.ts"]}}}"#,
            "typesVersions",
            "\"no\"",
        ),
    ] {
        let config = configuration("", &["src/main.ts"]);
        for dialect in DIALECTS {
            let fixture = Fixture::with_dialect(
                dialect,
                &[
                    ("tsconfig.json", &config),
                    ("node_modules/dup/package.json", manifest_text),
                    ("node_modules/dup/lib/sub.d.ts", text),
                    ("node_modules/a/package.json", &manifest("a", "1.0.0", "")),
                    (
                        "node_modules/a/index.d.ts",
                        "import { Box } from \"dup/sub\";\nexport declare const made: Box;\n",
                    ),
                    (
                        "node_modules/a/node_modules/dup/package.json",
                        manifest_text,
                    ),
                    ("node_modules/a/node_modules/dup/lib/sub.d.ts", text),
                    (
                        "src/main.ts",
                        "import { Box } from \"dup/sub\";\nimport { made } from \"a\";\nexport type SameBox = typeof made extends Box ? \"yes\" : \"no\";\n",
                    ),
                ],
            );
            let project = fixture.load();
            assert_eq!(
                summary(&project, &fixture, "SameBox"),
                expected,
                "{dialect:?}, {case}"
            );
        }
    }
}

/// A resolution failure is the failure of the file that asked for it: a
/// manifest this tool cannot read (`Io`), met only while resolving the
/// redirected copy's import, fails no load, as 5.6.3 never resolves a
/// redirected file's imports and 7.0.2 keeps none of them. The unreadable
/// manifest is a `broken` package beside the copy, holding nothing else, so
/// the lookup goes on to the readable `broken` above it. It fails the load
/// once the nested copy is a different version, so an input, and when a
/// file that stays in the program asks the same question after the
/// redirected copy asked it first, which the resolution's memo answers.
#[cfg(unix)]
#[test]
fn resolution_failures_only_a_redirected_copy_meets_are_not_reported() {
    use std::os::unix::fs::PermissionsExt;
    for (nested_version, shared, fails) in [
        ("1.0.0", false, false),
        ("2.0.0", false, true),
        ("1.0.0", true, true),
    ] {
        for dialect in DIALECTS {
            let dup = manifest("dup", "1.0.0", "");
            let nested = manifest("dup", nested_version, "");
            let config = configuration("", &["src/main.ts"]);
            let nested_copy = format!("import \"broken\";\n{}", copy("nested"));
            let main = if shared {
                "import { Box } from \"dup\";\nimport { made } from \"a\";\nimport \"./second\";\nexport const b = made;\n"
            } else {
                "import { Box } from \"dup\";\nimport { made } from \"a\";\nexport const b = made;\n"
            };
            let fixture = Fixture::with_dialect(
                dialect,
                &[
                    ("tsconfig.json", &config),
                    ("node_modules/dup/package.json", &dup),
                    ("node_modules/dup/index.d.ts", &copy("root")),
                    ("node_modules/a/package.json", &manifest("a", "1.0.0", "")),
                    (
                        "node_modules/a/index.d.ts",
                        "import { Box } from \"dup\";\nexport declare const made: Box;\n",
                    ),
                    ("node_modules/a/node_modules/dup/package.json", &nested),
                    ("node_modules/a/node_modules/dup/index.d.ts", &nested_copy),
                    (
                        "node_modules/a/node_modules/dup/other.d.ts",
                        "import \"broken\";\nexport {};\n",
                    ),
                    (
                        "node_modules/a/node_modules/dup/node_modules/broken/package.json",
                        &manifest("broken", "1.0.0", ""),
                    ),
                    (
                        "node_modules/a/node_modules/broken/package.json",
                        &manifest("broken", "1.0.0", ""),
                    ),
                    (
                        "node_modules/a/node_modules/broken/index.d.ts",
                        "export {};\n",
                    ),
                    ("src/main.ts", main),
                    (
                        "src/second.ts",
                        "import \"../node_modules/a/node_modules/dup/other\";\nexport {};\n",
                    ),
                ],
            );
            let unreadable =
                fixture.path("node_modules/a/node_modules/dup/node_modules/broken/package.json");
            std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000)).unwrap();
            if std::fs::read(&unreadable).is_ok() {
                eprintln!(
                    "SKIP resolution_failures_only_a_redirected_copy_meets_are_not_reported: this user reads files without permission"
                );
                return;
            }
            let result = fixture.try_load();
            std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o644)).unwrap();
            let context = format!("{dialect:?}, nested {nested_version}, shared {shared}");
            if fails {
                let error = result.expect_err(&context);
                assert_eq!(error.kind, InputErrorKind::Io, "{context}: {error}");
                assert_eq!(
                    error.path.as_deref(),
                    Some(unreadable.as_path()),
                    "{context}"
                );
            } else {
                result.unwrap_or_else(|error| panic!("{context}: {error}"));
            }
        }
    }
}

/// A manifest with an empty `name` gives a package id under 5.6.3, which
/// deduplicates the copies (`/index.d.ts@1.0.0`), and none that collection
/// uses under 7.0.2, which requires a nonempty name (`filesParser.start`,
/// `collectFiles`) and so keeps two identities, as its TS2322 shows.
#[test]
fn an_empty_package_name_deduplicates_only_under_5_6() {
    assert_eq!(
        header_identity(r#"{"name":"","version":"1.0.0","types":"index.d.ts"}"#),
        ["\"no\"", "\"yes\""]
    );
}

/// Repeated `name` and `version` fields, as each compiler decodes them:
/// 7.0.2's `Expected[string]` keeps the last string through a later value of
/// another type and loses it to a later `null`; 5.6.3's object keeps the
/// last value, which must be a string.
#[test]
fn repeated_manifest_fields_are_read_as_each_compiler_decodes_them() {
    for (manifest_text, expected) in [
        (
            r#"{"name":"dup","version":"1.0.0","version":42,"types":"index.d.ts"}"#,
            ["\"yes\"", "\"no\""],
        ),
        (
            r#"{"name":"dup","name":7,"version":"1.0.0","types":"index.d.ts"}"#,
            ["\"yes\"", "\"no\""],
        ),
        (
            r#"{"name":"dup","version":"1.0.0","version":null,"types":"index.d.ts"}"#,
            ["\"no\"", "\"no\""],
        ),
        (
            r#"{"name":"dup","version":42,"version":"1.0.0","types":"index.d.ts"}"#,
            ["\"yes\"", "\"yes\""],
        ),
    ] {
        assert_eq!(header_identity(manifest_text), expected, "{manifest_text}");
    }
    // A peer's version: 7.0.2 reads the retained strings `18.0.0` and
    // `19.0.0`, two identities; 5.6.3 reads `true` beside both copies, one.
    for dialect in DIALECTS {
        let mut files = peer_files(
            r#","peerDependencies":{"react":"*"}"#,
            r#","peerDependencies":{"react":"*"}"#,
            None,
            None,
        );
        for (directory, version) in [
            ("node_modules/react", "18.0.0"),
            ("node_modules/b/node_modules/react", "19.0.0"),
        ] {
            files.push((
                format!("{directory}/package.json"),
                format!(r#"{{"name":"react","version":"{version}","version":true}}"#),
            ));
        }
        let fixture = Fixture::with_dialect(dialect, &borrowed(&files));
        let project = fixture.load();
        let expected = match dialect {
            TypeScript7 => "\"no\"",
            TypeScript56 => "\"yes\"",
        };
        assert_eq!(
            summary(&project, &fixture, "SameP"),
            expected,
            "{dialect:?}"
        );
    }
}

/// Whether two copies of `dup` with `manifest_text` are one identity, under
/// TypeScript 7 and then TypeScript 5.6.
fn header_identity(manifest_text: &str) -> [String; 2] {
    let config = configuration("", &["src/main.ts"]);
    let text = "export declare class Box { protected secret: number; }\n";
    DIALECTS.map(|dialect| {
        let fixture = Fixture::with_dialect(
            dialect,
            &[
                ("tsconfig.json", &config),
                ("node_modules/dup/package.json", manifest_text),
                ("node_modules/dup/index.d.ts", text),
                ("node_modules/a/package.json", &manifest("a", "1.0.0", "")),
                (
                    "node_modules/a/index.d.ts",
                    "import { Box } from \"dup\";\nexport declare const made: Box;\n",
                ),
                ("node_modules/a/node_modules/dup/package.json", manifest_text),
                ("node_modules/a/node_modules/dup/index.d.ts", text),
                (
                    "src/main.ts",
                    "import { Box } from \"dup\";\nimport { made } from \"a\";\nexport type SameBox = typeof made extends Box ? \"yes\" : \"no\";\n",
                ),
            ],
        );
        let project = fixture.try_load().unwrap_or_else(|error| panic!("{dialect:?}: {error}"));
        summary(&project, &fixture, "SameBox")
    })
}

/// A session re-answers when the link to a copy's directory is retargeted:
/// the nested copy's real directory, and so the `react` beside it, change
/// while every file keeps its text, and each answer equals a fresh load, with
/// and without `preserveSymlinks`.
#[cfg(unix)]
#[test]
fn a_session_refreshes_when_a_copys_directory_link_is_retargeted() {
    use typescript_parser::analysis::AnalysisError;
    use typescript_parser::project::{ProjectRequest, ProjectResponse, ProjectSession};
    let peer_manifest = manifest("peer", "1.0.0", r#","peerDependencies":{"react":"*"}"#);
    let peer = "export declare class P { protected p: number; }\n";
    let react = |version: &str| format!(r#"{{"name":"react","version":"{version}"}}"#);
    // Under `preserveSymlinks` the files keep their link spellings, so only
    // the real directory the peers are read beside sees the retarget.
    for (dialect, preserve) in DIALECTS
        .into_iter()
        .flat_map(|dialect| [(dialect, ""), (dialect, r#","preserveSymlinks":true"#)])
    {
        let config = configuration(preserve, &["src/main.ts"]);
        let fixture = Fixture::with_dialect(
            dialect,
            &[
                ("tsconfig.json", &config),
                ("node_modules/react/package.json", &react("18.0.0")),
                ("node_modules/peer/package.json", &peer_manifest),
                ("node_modules/peer/index.d.ts", peer),
                ("node_modules/b/package.json", &manifest("b", "1.0.0", "")),
                (
                    "node_modules/b/index.d.ts",
                    "import { P } from \"peer\";\nexport declare const madeP: P;\n",
                ),
                (".store/same/node_modules/peer/package.json", &peer_manifest),
                (".store/same/node_modules/peer/index.d.ts", peer),
                (
                    ".store/same/node_modules/react/package.json",
                    &react("18.0.0"),
                ),
                (
                    ".store/other/node_modules/peer/package.json",
                    &peer_manifest,
                ),
                (".store/other/node_modules/peer/index.d.ts", peer),
                (
                    ".store/other/node_modules/react/package.json",
                    &react("19.0.0"),
                ),
                (
                    "src/main.ts",
                    "import { P } from \"peer\";\nimport { madeP } from \"b\";\nexport type SameP = typeof madeP extends P ? \"yes\" : \"no\";\n",
                ),
            ],
        );
        let link = fixture.path("node_modules/b/node_modules/peer");
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        let point = |store: &str| {
            let _ = std::fs::remove_file(&link);
            std::os::unix::fs::symlink(format!("../../../.store/{store}/node_modules/peer"), &link)
                .unwrap();
        };
        point("same");
        let path = fixture.path("tsconfig.json");
        let request = || {
            ProjectRequest::Type(TypeRequest::new(TypeQuery::Name {
                name: "SameP".into(),
                file: Some(fixture.path("src/main.ts")),
            }))
        };
        let summary = |response: Result<ProjectResponse, AnalysisError>| match response {
            Ok(ProjectResponse::Type(answer)) => match *answer {
                TypeAnswer::Selected(result) => result.description.summary,
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        };
        let mut session = ProjectSession::new(&path).unwrap();
        for (store, expected) in [
            ("same", "\"yes\""),
            ("other", "\"no\""),
            ("same", "\"yes\""),
        ] {
            point(store);
            let refreshed = summary(session.execute(request()));
            let fresh =
                summary(Project::load(&path).and_then(|project| project.execute(request())));
            assert_eq!(refreshed, fresh, "{dialect:?}{preserve}, linked to {store}");
            assert_eq!(
                refreshed, expected,
                "{dialect:?}{preserve}, linked to {store}"
            );
        }
    }
}

/// Whether a file is a project file follows the routes each compiler
/// counts: `src/main.ts` imports the root `dup`, then `a`, whose nested copy
/// redirects to the root copy, then `shared`; a later root, `src/refs.ts`,
/// references the nested copy by path, and that copy references `shared`.
/// 7.0.2 counts the redirect's own reference (`lowestDepth`), so `shared` is
/// a project file; 5.6.3 processes no redirect's subtasks
/// (`sourceFilesFoundSearchingNodeModules`), so `shared` stays a
/// dependency, which a project query does not answer for.
#[test]
fn provenance_follows_what_each_dialects_collection_processes() {
    use typescript_parser::analysis::{AnalysisError, QueryErrorKind};
    let dup = manifest("dup", "1.0.0", "");
    let a = manifest("a", "1.0.0", "");
    let shared = manifest("shared", "1.0.0", "");
    let config = configuration("", &["src/main.ts", "src/refs.ts"]);
    let layout = |root_copy: &'static str| {
        vec![
            ("tsconfig.json", config.clone()),
            ("node_modules/dup/package.json", dup.clone()),
            ("node_modules/dup/index.d.ts", root_copy.to_owned()),
            ("node_modules/dup/note.d.ts", "declare const rootNote: 1;\n".to_owned()),
            ("node_modules/a/package.json", a.clone()),
            ("node_modules/a/index.d.ts", "import \"dup\";\nexport {};\n".to_owned()),
            ("node_modules/a/node_modules/dup/package.json", dup.clone()),
            (
                "node_modules/a/node_modules/dup/index.d.ts",
                "/// <reference path=\"../../../shared/index.d.ts\" />\nexport declare const which: \"nested\";\n".to_owned(),
            ),
            (
                "node_modules/a/node_modules/dup/note.d.ts",
                "declare const nestedNote: 1;\n".to_owned(),
            ),
            ("node_modules/shared/package.json", shared.clone()),
            (
                "node_modules/shared/index.d.ts",
                "export declare const shared: number;\n".to_owned(),
            ),
            (
                "src/main.ts",
                "import \"dup\";\nimport \"a\";\nimport \"shared\";\nexport {};\n".to_owned(),
            ),
            (
                "src/refs.ts",
                "/// <reference path=\"../node_modules/a/node_modules/dup/index.d.ts\" />\nexport {};\n".to_owned(),
            ),
        ]
    };
    let declarations = layout("export declare const which: \"root\";\n");
    for dialect in DIALECTS {
        let files: Vec<(&str, &str)> = declarations
            .iter()
            .map(|(path, text)| (*path, text.as_str()))
            .collect();
        let fixture = Fixture::with_dialect(dialect, &files);
        let project = fixture
            .try_load()
            .unwrap_or_else(|error| panic!("{dialect:?}: {error}"));
        let outline = project.outline(&fixture.path("node_modules/shared/index.d.ts"));
        match dialect {
            TypeScript7 => {
                let outline = outline.unwrap_or_else(|error| panic!("{dialect:?}: {error}"));
                let names: Vec<&str> = outline
                    .entries
                    .iter()
                    .map(|entry| entry.label.as_str())
                    .collect();
                assert_eq!(names, ["shared"], "{dialect:?}");
            }
            TypeScript56 => {
                let error = match outline {
                    Err(AnalysisError::Query(error)) => error,
                    other => panic!("{dialect:?}: {other:?}"),
                };
                assert_eq!(error.kind, QueryErrorKind::OutsideProject, "{dialect:?}");
            }
        }
    }
}

/// Two identical copies of `dup` reference their own `./note.d.ts`, which
/// declares a global interface: the root copy's declares `root`, the nested
/// copy's `nested`. `src/refs.ts` references the nested copy, which
/// redirects to the root copy. 5.6.3 processes the redirect again there,
/// following the root copy's reference from the nested copy's location, so
/// the nested `note.d.ts` is a project input and `Notes` has `nested`
/// (`HasNested` is `"yes"`, which `const check: HasNested = "yes"` shows
/// without an error); 7.0.2 never reaches it (TS2322 there: `"no"`). Where
/// the nested `note.d.ts` is missing, 5.6.3 reports TS6053 at the nested
/// copy unless `skipLibCheck` skips it, and 7.0.2 reports nothing. A
/// session follows an edit to the nested `note.d.ts`, and its removal, as a
/// fresh load does.
#[test]
fn a_redirect_typescript_5_6_processes_again_follows_its_targets_references() {
    use typescript_parser::analysis::{AnalysisError, QueryErrorKind};
    use typescript_parser::project::{ProjectRequest, ProjectResponse, ProjectSession};
    let dup = manifest("dup", "1.0.0", "");
    let a = manifest("a", "1.0.0", "");
    let copy = "/// <reference path=\"./note.d.ts\" />\nexport declare const which: \"copy\";\n";
    let files = |extra: &str, nested_note: bool| {
        let mut files = vec![
            (
                "tsconfig.json",
                configuration(extra, &["src/main.ts", "src/refs.ts"]),
            ),
            ("node_modules/dup/package.json", dup.clone()),
            ("node_modules/dup/index.d.ts", copy.to_owned()),
            (
                "node_modules/dup/note.d.ts",
                "interface Notes { root: 1; }\n".to_owned(),
            ),
            ("node_modules/a/package.json", a.clone()),
            (
                "node_modules/a/index.d.ts",
                "import \"dup\";\nexport {};\n".to_owned(),
            ),
            ("node_modules/a/node_modules/dup/package.json", dup.clone()),
            ("node_modules/a/node_modules/dup/index.d.ts", copy.to_owned()),
            (
                "src/main.ts",
                "import \"dup\";\nimport \"a\";\nexport type HasNested = \"nested\" extends keyof Notes ? \"yes\" : \"no\";\n".to_owned(),
            ),
            (
                "src/refs.ts",
                "/// <reference path=\"../node_modules/a/node_modules/dup/index.d.ts\" />\nexport {};\n".to_owned(),
            ),
        ];
        if nested_note {
            files.push((
                "node_modules/a/node_modules/dup/note.d.ts",
                "interface Notes { nested: 1; }\n".to_owned(),
            ));
        }
        files
    };
    for dialect in DIALECTS {
        let all = files("", true);
        let all: Vec<(&str, &str)> = all
            .iter()
            .map(|(path, text)| (*path, text.as_str()))
            .collect();
        let fixture = Fixture::with_dialect(dialect, &all);
        let project = fixture
            .try_load()
            .unwrap_or_else(|error| panic!("{dialect:?}: {error}"));
        let note = fixture.path("node_modules/a/node_modules/dup/note.d.ts");
        match dialect {
            TypeScript56 => {
                assert_eq!(summary(&project, &fixture, "HasNested"), "\"yes\"");
                let outline = project
                    .outline(&note)
                    .unwrap_or_else(|error| panic!("{dialect:?}: {error}"));
                let names: Vec<&str> = outline
                    .entries
                    .iter()
                    .map(|entry| entry.label.as_str())
                    .collect();
                assert_eq!(names, ["Notes", "nested"]);
            }
            TypeScript7 => {
                assert_eq!(summary(&project, &fixture, "HasNested"), "\"no\"");
                let error = match project.outline(&note) {
                    Err(AnalysisError::Query(error)) => error,
                    other => panic!("{dialect:?}: {other:?}"),
                };
                assert_eq!(error.kind, QueryErrorKind::OutsideProject);
            }
        }
        // A session refreshes what only the redirect's processing reaches.
        let path = fixture.path("tsconfig.json");
        let request = || {
            ProjectRequest::Type(TypeRequest::new(TypeQuery::Name {
                name: "HasNested".into(),
                file: Some(fixture.path("src/main.ts")),
            }))
        };
        let answer = |response: Result<ProjectResponse, AnalysisError>| match response {
            Ok(ProjectResponse::Type(answer)) => match *answer {
                TypeAnswer::Selected(result) => Ok(result.description.summary),
                other => panic!("{other:?}"),
            },
            Err(AnalysisError::Input(error)) => Err(error.kind),
            other => panic!("{other:?}"),
        };
        let mut session = ProjectSession::new(&path).unwrap();
        let mut ask = |step: &str, expected: Result<&str, InputErrorKind>| {
            let refreshed = answer(session.execute(request()));
            let fresh = answer(Project::load(&path).and_then(|project| project.execute(request())));
            assert_eq!(refreshed, fresh, "{dialect:?}, {step}");
            assert_eq!(
                refreshed,
                expected.map(str::to_owned),
                "{dialect:?}, {step}"
            );
        };
        let nested_answer = |answer: &'static str| match dialect {
            TypeScript56 => answer,
            TypeScript7 => "\"no\"",
        };
        ask("loaded", Ok(nested_answer("\"yes\"")));
        std::fs::write(&note, "interface Notes { other: 1; }\n").unwrap();
        ask("nested note edited", Ok("\"no\""));
        std::fs::remove_file(&note).unwrap();
        ask(
            "nested note removed",
            match dialect {
                TypeScript56 => Err(InputErrorKind::UnresolvedImport),
                TypeScript7 => Ok("\"no\""),
            },
        );
        std::fs::write(&note, "interface Notes { nested: 1; }\n").unwrap();
        ask("nested note restored", Ok(nested_answer("\"yes\"")));
        // The nested `note.d.ts` missing.
        for (extra, reported) in [("", true), (r#","skipLibCheck":true"#, false)] {
            let missing = files(extra, false);
            let missing: Vec<(&str, &str)> = missing
                .iter()
                .map(|(path, text)| (*path, text.as_str()))
                .collect();
            let fixture = Fixture::with_dialect(dialect, &missing);
            let context = format!("{dialect:?}{extra}");
            match fixture.try_load() {
                Err(error) => {
                    assert!(dialect == TypeScript56 && reported, "{context}: {error}");
                    assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{context}");
                    assert_eq!(
                        error.path.as_deref(),
                        Some(
                            fixture
                                .path("node_modules/a/node_modules/dup/index.d.ts")
                                .as_path()
                        ),
                        "{context}: {error}"
                    );
                    assert!(
                        error.message.contains("'./note.d.ts'"),
                        "{context}: {error}"
                    );
                }
                Ok(_) => assert!(dialect == TypeScript7 || !reported, "{context}"),
            }
        }
    }
}

/// A module augmentation adds no file (`shouldAddFile`), so what it names
/// is known only once collection has added every file: here the nested
/// copy's `note.d.ts`, which only 5.6.3's processing of the redirect again
/// adds, from the root copy's reference, where `src/refs.ts` references the
/// nested copy, which itself references nothing. 5.6.3 then augments it, so
/// `both` is `1 | 2` there with no diagnostic; 7.0.2 never adds it and
/// reports TS2664 at the augmentation.
#[test]
fn an_augmentation_names_a_file_typescript_5_6_adds_processing_a_redirect_again() {
    let dup = manifest("dup", "1.0.0", "");
    let a = manifest("a", "1.0.0", "");
    let config = configuration("", &["src/main.ts", "src/refs.ts"]);
    let files = [
        ("tsconfig.json", config.as_str()),
        ("node_modules/dup/package.json", &dup),
        (
            "node_modules/dup/index.d.ts",
            "/// <reference path=\"./note.d.ts\" />\nexport declare const which: \"root\";\n",
        ),
        (
            "node_modules/dup/note.d.ts",
            "export interface Notes { root: 1; }\n",
        ),
        ("node_modules/a/package.json", &a),
        ("node_modules/a/index.d.ts", "import \"dup\";\nexport {};\n"),
        ("node_modules/a/node_modules/dup/package.json", &dup),
        (
            "node_modules/a/node_modules/dup/index.d.ts",
            "export declare const which: \"nested\";\n",
        ),
        (
            "node_modules/a/node_modules/dup/note.d.ts",
            "export interface Notes { original: 1; }\nexport declare const both: Notes[\"original\"] | Notes[\"augmented\"];\n",
        ),
        (
            "src/main.ts",
            "import \"dup\";\nimport \"a\";\ndeclare module \"../node_modules/a/node_modules/dup/note\" {\n  interface Notes { augmented: 2; }\n}\nexport {};\n",
        ),
        (
            "src/refs.ts",
            "/// <reference path=\"../node_modules/a/node_modules/dup/index.d.ts\" />\nexport {};\n",
        ),
    ];
    for dialect in DIALECTS {
        let fixture = Fixture::with_dialect(dialect, &files);
        match dialect {
            TypeScript56 => {
                let project = fixture.load();
                let answer = project
                    .type_of(TypeRequest {
                        query: TypeQuery::Name {
                            name: "both".to_owned(),
                            file: Some(fixture.path("node_modules/a/node_modules/dup/note.d.ts")),
                        },
                        expand: false,
                        depth: 0,
                    })
                    .unwrap_or_else(|error| panic!("{error}"));
                let TypeAnswer::Selected(answer) = answer else {
                    panic!("{answer:?}");
                };
                assert_eq!(answer.description.summary, "1 | 2");
            }
            TypeScript7 => {
                let error = fixture.try_load().expect_err("7.0.2 reports TS2664");
                assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");
                assert_eq!(
                    error.path.as_deref(),
                    Some(fixture.path("src/main.ts").as_path()),
                    "{error}"
                );
            }
        }
    }
}

/// A lookup that 5.6.3's processing of a redirect again makes, resolving
/// the root copy's `import "broken"` from the nested copy's location,
/// cannot read the `package.json` beside the nested copy: under 5.6 it fails
/// the load at once, as a file processed at depth zero that stays in the
/// program, ahead of an input that `src/main.ts` imports first and that
/// cannot be read either; under 7 no file makes that lookup, and that input's
/// failure, where there is one, is the load's.
#[cfg(unix)]
#[test]
fn a_lookup_failure_processing_a_redirect_again_fails_the_load_under_5_6() {
    use std::os::unix::fs::PermissionsExt;
    for early_failure in [false, true] {
        for dialect in DIALECTS {
            let dup = manifest("dup", "1.0.0", "");
            let broken = manifest("broken", "1.0.0", "");
            let config = configuration("", &["src/main.ts", "src/refs.ts"]);
            let fixture = Fixture::with_dialect(
                dialect,
                &[
                    ("tsconfig.json", &config),
                    (
                        "node_modules/early/package.json",
                        &manifest("early", "1.0.0", ""),
                    ),
                    ("node_modules/early/index.d.ts", "export {};\n"),
                    ("node_modules/dup/package.json", &dup),
                    (
                        "node_modules/dup/index.d.ts",
                        "import \"broken\";\nexport declare const which: \"root\";\n",
                    ),
                    ("node_modules/broken/package.json", &broken),
                    ("node_modules/broken/index.d.ts", "export {};\n"),
                    ("node_modules/a/package.json", &manifest("a", "1.0.0", "")),
                    ("node_modules/a/index.d.ts", "import \"dup\";\nexport {};\n"),
                    ("node_modules/a/node_modules/dup/package.json", &dup),
                    (
                        "node_modules/a/node_modules/dup/index.d.ts",
                        "export declare const which: \"nested\";\n",
                    ),
                    (
                        "node_modules/a/node_modules/dup/node_modules/broken/package.json",
                        &broken,
                    ),
                    (
                        "src/main.ts",
                        "import \"early\";\nimport \"dup\";\nimport \"a\";\nexport {};\n",
                    ),
                    (
                        "src/refs.ts",
                        "/// <reference path=\"../node_modules/a/node_modules/dup/index.d.ts\" />\nexport {};\n",
                    ),
                ],
            );
            let unreadable =
                fixture.path("node_modules/a/node_modules/dup/node_modules/broken/package.json");
            let early = fixture.path("node_modules/early/index.d.ts");
            let mut locked = vec![unreadable.clone()];
            if early_failure {
                locked.push(early.clone());
            }
            for file in &locked {
                std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o000)).unwrap();
            }
            if std::fs::read(&unreadable).is_ok() {
                eprintln!(
                    "SKIP a_lookup_failure_processing_a_redirect_again_fails_the_load_under_5_6: this user reads files without permission"
                );
                return;
            }
            let result = fixture.try_load();
            for file in &locked {
                std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o644)).unwrap();
            }
            let context = format!("{dialect:?}, early failure {early_failure}");
            let failed = match (dialect, early_failure) {
                (TypeScript56, _) => Some(&unreadable),
                (TypeScript7, true) => Some(&early),
                (TypeScript7, false) => None,
            };
            match failed {
                Some(failed) => {
                    let error = result.expect_err(&context);
                    assert_eq!(error.kind, InputErrorKind::Io, "{context}: {error}");
                    assert_eq!(
                        error.path.as_deref(),
                        Some(failed.as_path()),
                        "{context}: {error}"
                    );
                }
                None => {
                    result.unwrap_or_else(|error| panic!("{context}: {error}"));
                }
            }
        }
    }
}

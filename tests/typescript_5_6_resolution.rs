//! Module resolution under the TypeScript 5.6 dialect beside the same
//! request under TypeScript 7, at the final outcome: the file each import
//! names, as the type of a value it exports shows. Every target declares
//! `which` as its own path. Expected files are 5.6.3's and native 7.0.2's
//! (`tsc --listFilesOnly --traceResolution --incremental false` on scratch
//! copies).

mod support;

use std::fmt::Write as _;
use support::Dialect::{self, TypeScript7, TypeScript56};
use support::{BASE_OPTIONS, DIALECTS, Fixture};
use typescript_parser::analysis::InputErrorKind;
use typescript_parser::project::Project;
use typescript_parser::types::{TypeAnswer, TypeQuery, TypeRequest};

/// A module that names itself: `which` is its path.
fn labelled(path: &str) -> String {
    format!("export declare const which: \"{path}\";\n")
}

/// `files`, each path in `labelled` also declaring itself, and `dialect`'s
/// compiler.
fn scratch(dialect: Dialect, files: &[(&str, &str)], labelled_paths: &[&str]) -> Fixture {
    let labels: Vec<(String, String)> = labelled_paths
        .iter()
        .map(|path| ((*path).to_owned(), labelled(path)))
        .collect();
    let mut all: Vec<(&str, &str)> = files.to_vec();
    all.extend(
        labels
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str())),
    );
    Fixture::with_dialect(dialect, &all)
}

/// The importer text that imports `which` from each specifier, in order,
/// and exports it as `v<index>`.
fn importer(specifiers: &[&str]) -> String {
    let mut text = String::new();
    for (index, specifier) in specifiers.iter().enumerate() {
        writeln!(
            text,
            "import {{ which as w{index} }} from {specifier:?};\nexport const v{index} = w{index};"
        )
        .unwrap();
    }
    text
}

/// The label each of `count` imports reached: the summary of `v<index>`'s
/// type, the quoted path of the file it names.
fn reached(project: &Project, count: usize) -> Vec<String> {
    (0..count)
        .map(|index| {
            let answer = project
                .type_of(TypeRequest {
                    query: TypeQuery::Name {
                        name: format!("v{index}"),
                        file: None,
                    },
                    expand: false,
                    depth: 0,
                })
                .unwrap_or_else(|error| panic!("v{index}: {error}"));
            let TypeAnswer::Selected(answer) = answer else {
                panic!("v{index}: {answer:?}");
            };
            let summary = answer.description.summary;
            summary
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .unwrap_or_else(|| panic!("v{index}: {summary}"))
                .to_owned()
        })
        .collect()
}

/// The targets of the precedence experiments, at the project root and in
/// `node_modules`.
const TARGETS: &[&str] = &[
    "src/x.ts",
    "pkg/sub.ts",
    "testing/imports.ts",
    "exact.ts",
    "wild/tra.ts",
    "wild/act.ts",
    "first/xc.ts",
    "second/x.ts",
    "first/zc.ts",
    "second/z.ts",
    "gone.ts",
    "#internal.ts",
    "src/imp.ts",
    "bare-dir/lib.d.ts",
    "bare-dir/index.ts",
    "node_modules/pkg/sub.d.ts",
    "node_modules/testing/imports.d.ts",
    "node_modules/gone/index.d.ts",
    "node_modules/nothing/index.d.ts",
    "node_modules/bare-dir/index.d.ts",
];

/// One import per rule `tryLoadModuleUsingOptionalResolutionSettings`
/// distinguishes, from a file at the project root.
const SPECIFIERS: &[&str] = &[
    // A wildcard pattern's substitution exists.
    "@/x",
    // A matched pattern whose substitution is missing ends the optional
    // settings: `pkg/sub.ts` under `baseUrl` is not tried, and the package
    // is.
    "pkg/sub",
    // No pattern matches: `baseUrl/testing/imports` before the package.
    "testing/imports",
    // The same name with a backslash, which the candidate path normalizes.
    "testing\\imports",
    // An exact key before a wildcard one that also matches.
    "exact",
    "extra",
    // Equal prefixes: the first pattern written wins, in either order.
    "abxc",
    "xyzc",
    // An exact key whose substitution is missing.
    "gone",
    // Unmatched and absent under `baseUrl`.
    "nothing",
    // A directory under `baseUrl` with a manifest.
    "bare-dir",
    // `baseUrl` precedes package `imports`.
    "#internal",
];

const PATTERNS: &str = r#""@/*":["SRC*"],"pkg/*":["./shim/*"],"exact":["./exact.ts"],"ex*":["./wild/*"],"ab*":["./first/*"],"ab*c":["./second/*"],"xy*c":["./second/*"],"xy*":["./first/*"],"gone":["./gone-target.ts"]"#;

/// Under 5.6, a name `paths` matches resolves through its pattern alone,
/// missing substitutions included, then through packages; any other bare
/// name is first looked up under `baseUrl`, before package `imports` and
/// packages. TypeScript 7 has no `baseUrl`: the same names fall to
/// packages and `imports`.
#[test]
fn base_url_resolves_bare_names_no_paths_pattern_matches() {
    for (dialect, options, expected) in [
        (
            TypeScript56,
            format!(
                r#"{BASE_OPTIONS},"baseUrl":".","paths":{{{}}}"#,
                PATTERNS.replace("SRC", "src/")
            ),
            [
                "src/x.ts",
                "node_modules/pkg/sub.d.ts",
                "testing/imports.ts",
                "testing/imports.ts",
                "exact.ts",
                "wild/tra.ts",
                "first/xc.ts",
                "second/z.ts",
                "node_modules/gone/index.d.ts",
                "node_modules/nothing/index.d.ts",
                "bare-dir/lib.d.ts",
                "#internal.ts",
            ],
        ),
        (
            TypeScript7,
            format!(
                r#"{BASE_OPTIONS},"paths":{{{}}}"#,
                PATTERNS.replace("SRC", "./src/")
            ),
            [
                "src/x.ts",
                "node_modules/pkg/sub.d.ts",
                "node_modules/testing/imports.d.ts",
                "node_modules/testing/imports.d.ts",
                "exact.ts",
                "wild/tra.ts",
                "first/xc.ts",
                "second/z.ts",
                "node_modules/gone/index.d.ts",
                "node_modules/nothing/index.d.ts",
                "node_modules/bare-dir/index.d.ts",
                "src/imp.ts",
            ],
        ),
    ] {
        let config = format!(r#"{{"compilerOptions":{{{options}}},"files":["main.ts"]}}"#);
        let main = importer(SPECIFIERS);
        let fixture = scratch(
            dialect,
            &[
                ("tsconfig.json", &config),
                ("main.ts", &main),
                (
                    "package.json",
                    r##"{"imports":{"#internal":"./src/imp.ts"}}"##,
                ),
                ("bare-dir/package.json", r#"{"types":"lib.d.ts"}"#),
            ],
            TARGETS,
        );
        let project = fixture.load();
        assert_eq!(reached(&project, SPECIFIERS.len()), expected, "{dialect:?}");
    }
}

/// `baseUrl` and the `paths` base come from the effective options: a
/// `baseUrl` a base configuration declares is relative to that file, and
/// `paths` a base declares are relative to the leaf's `baseUrl` when the
/// leaf sets one (`getPathsBasePath`); `${configDir}` names the leaf.
#[test]
fn base_url_and_paths_bases_follow_their_declaring_files() {
    let options = BASE_OPTIONS;
    for (name, base, leaf_options, expected) in [
        (
            "baseUrl and paths in the base",
            r#"{"compilerOptions":{"baseUrl":"./lib","paths":{"@/*":["src/*"]}}}"#,
            "",
            ["lib/src/x.ts", "lib/thing.ts"],
        ),
        (
            "paths in the base, baseUrl in the leaf",
            r#"{"compilerOptions":{"paths":{"@/*":["./src/*"]}}}"#,
            r#","baseUrl":"./here""#,
            ["leaf/here/src/x.ts", "leaf/here/thing.ts"],
        ),
        (
            "baseUrl in the base, paths in the leaf",
            r#"{"compilerOptions":{"baseUrl":"."}}"#,
            r#","paths":{"@/*":["./src/*"]}"#,
            ["src/x.ts", "thing.ts"],
        ),
        (
            "baseUrl through ${configDir} in the base",
            r#"{"compilerOptions":{"baseUrl":"${configDir}","paths":{"@/*":["src/*"]}}}"#,
            "",
            ["leaf/src/x.ts", "leaf/thing.ts"],
        ),
    ] {
        let leaf = format!(
            r#"{{"extends":"../base.json","compilerOptions":{{{options}{leaf_options}}},"files":["main.ts"]}}"#
        );
        let main = importer(&["@/x", "thing"]);
        let fixture = scratch(
            TypeScript56,
            &[
                ("base.json", base),
                ("leaf/tsconfig.json", &leaf),
                ("leaf/main.ts", &main),
            ],
            &[
                "lib/src/x.ts",
                "lib/thing.ts",
                "src/x.ts",
                "thing.ts",
                "leaf/here/src/x.ts",
                "leaf/here/thing.ts",
                "leaf/src/x.ts",
                "leaf/thing.ts",
            ],
        );
        let project = fixture
            .try_load_config("leaf/tsconfig.json")
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(reached(&project, 2), expected, "{name}");
    }
}

/// Relative and rooted names never consult `baseUrl`. A rooted name that a
/// `paths` pattern matches, with a missing substitution, then tries
/// `rootDirs` under TypeScript 7 and ends the optional settings under 5.6,
/// where `tryLoadModuleUsingPaths` answers for a matched pattern.
#[test]
fn relative_and_rooted_names_skip_base_url_and_a_matched_pattern_decides_under_5_6() {
    // `./only-at-base` exists under `baseUrl` but not beside the importer.
    let config = format!(
        r#"{{"compilerOptions":{{{BASE_OPTIONS},"baseUrl":"."}},"files":["src/main.ts"]}}"#
    );
    let main = importer(&["./only-at-base"]);
    let fixture = scratch(
        TypeScript56,
        &[("tsconfig.json", &config), ("src/main.ts", &main)],
        &["only-at-base.ts"],
    );
    let error = fixture
        .try_load()
        .expect_err("a relative name used baseUrl");
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}");

    for (dialect, expected) in [(TypeScript7, Some("gen/r.ts")), (TypeScript56, None)] {
        let config = format!(
            r#"{{"compilerOptions":{{{BASE_OPTIONS},"paths":{{"*":["./nowhere/*"]}},"rootDirs":["src","gen"]}},"files":["src/main.ts"]}}"#
        );
        let fixture = scratch(
            dialect,
            &[("tsconfig.json", config.as_str()), ("src/main.ts", "")],
            &["gen/r.ts"],
        );
        let root = std::fs::canonicalize(&fixture.root).unwrap();
        let rooted = format!("{}/src/r", root.to_str().unwrap());
        std::fs::write(fixture.path("src/main.ts"), importer(&[&rooted])).unwrap();
        match (fixture.try_load(), expected) {
            (Ok(project), Some(expected)) => {
                assert_eq!(reached(&project, 1), [expected], "{dialect:?}")
            }
            (Err(error), None) => {
                assert_eq!(error.kind, InputErrorKind::UnresolvedImport, "{error}")
            }
            (result, _) => panic!("{dialect:?}: {:?}", result.map(|_| ())),
        }
    }
}

/// `typesVersions` paths answer for a matched pattern the same way: under
/// 5.6 a matched pattern whose substitution is missing ends the package's
/// lookup, so neither the subpath's own file nor the `types` entry is
/// tried; TypeScript 7 continues past it.
#[test]
fn a_matched_types_versions_pattern_decides_under_5_6() {
    for (specifier, manifest) in [
        (
            "pkg/sub",
            r#"{"name":"pkg","version":"1.0.0","typesVersions":{"*":{"sub":["missing/sub.d.ts"]}}}"#,
        ),
        (
            "pkg",
            r#"{"name":"pkg","version":"1.0.0","types":"index.d.ts","typesVersions":{"*":{"index.d.ts":["missing/index.d.ts"]}}}"#,
        ),
    ] {
        for dialect in DIALECTS {
            let config = format!(r#"{{"compilerOptions":{{{BASE_OPTIONS}}},"files":["main.ts"]}}"#);
            let main = importer(&[specifier]);
            let fixture = scratch(
                dialect,
                &[
                    ("tsconfig.json", &config),
                    ("main.ts", &main),
                    ("node_modules/pkg/package.json", manifest),
                ],
                &["node_modules/pkg/sub.d.ts", "node_modules/pkg/index.d.ts"],
            );
            let context = format!("{dialect:?} {specifier}");
            match fixture.try_load() {
                Ok(project) if dialect == TypeScript7 => {
                    let expected = match specifier {
                        "pkg" => "node_modules/pkg/index.d.ts",
                        _ => "node_modules/pkg/sub.d.ts",
                    };
                    assert_eq!(reached(&project, 1), [expected], "{context}");
                }
                Err(error) if dialect == TypeScript56 => {
                    assert_eq!(
                        error.kind,
                        InputErrorKind::UnresolvedImport,
                        "{context}: {error}"
                    )
                }
                result => panic!("{context}: {:?}", result.map(|_| ())),
            }
        }
    }
}

/// A matched pattern's substitution, in `paths` and in a package's
/// `typesVersions` (a directory's manifest entry and a subpath in one
/// `node_modules`). 5.6.3's `tryLoadModuleUsingPaths` replaces the `*` only
/// with a non-empty capture (`matchedStar ? replaceFirstStar(...) : subst`),
/// so an empty one leaves the substitution as written, `*` included, which
/// names a file literally called `*`; TypeScript 7.0.2 always replaces it,
/// so the empty capture names the directory. A non-empty capture is
/// replaced in both. Expected files are 5.6.3's and native 7.0.2's
/// (`tsc --listFilesOnly --traceResolution --incremental false`), with and
/// without the files named `*`; `None` is unresolved.
#[test]
fn an_empty_wildcard_capture_keeps_the_star_only_under_5_6() {
    const PATHS: &str = r#""paths":{"ab*":["./first/*"],"thing*":["./shim*"]}"#;
    const LABELS: &[&str] = &[
        "first/index.ts",
        "first/x.ts",
        "shim.ts",
        "node_modules/thing/index.d.ts",
        "node_modules/pkg/index.d.ts",
        "node_modules/pkg/sub.d.ts",
        "node_modules/pkg/ts56/index.d.ts",
        "node_modules/pkg/ts56/x.d.ts",
        "node_modules/pkg2/index.d.ts",
        "node_modules/pkg2/ts56/ex.d.ts",
    ];
    const STARS: &[&str] = &["first/*.ts", "shim*.ts", "node_modules/pkg/ts56/*.d.ts"];
    let manifests = [
        (
            "node_modules/pkg/package.json",
            r#"{"name":"pkg","version":"1.0.0","typesVersions":{"*":{"sub*":["ts56/*"],"index*":["ts56/*"]}}}"#,
        ),
        (
            "node_modules/pkg2/package.json",
            r#"{"name":"pkg2","version":"1.0.0","typesVersions":{"*":{"ind*":["ts56/*"]}}}"#,
        ),
    ];
    // Each specifier's file under 5.6 and under TypeScript 7.
    /// The file reached without and with the files named `*`.
    type Reached = [Option<&'static str>; 2];
    let cases: &[(&str, Reached, Reached)] = &[
        // `paths`: empty and non-empty captures.
        (
            "ab",
            [None, Some("first/*.ts")],
            [Some("first/index.ts"); 2],
        ),
        ("abx", [Some("first/x.ts"); 2], [Some("first/x.ts"); 2]),
        // The matched pattern ends the optional settings under 5.6, and the
        // package follows.
        (
            "thing",
            [Some("node_modules/thing/index.d.ts"), Some("shim*.ts")],
            [Some("shim.ts"); 2],
        ),
        // A directory's manifest entry: `index` matches `index*` (empty) or
        // `ind*` (`ex`).
        (
            "pkg",
            [None, Some("node_modules/pkg/ts56/*.d.ts")],
            [Some("node_modules/pkg/ts56/index.d.ts"); 2],
        ),
        (
            "pkg2",
            [Some("node_modules/pkg2/ts56/ex.d.ts"); 2],
            [Some("node_modules/pkg2/ts56/ex.d.ts"); 2],
        ),
        // A subpath in one `node_modules`.
        (
            "pkg/sub",
            [None, Some("node_modules/pkg/ts56/*.d.ts")],
            [Some("node_modules/pkg/ts56/index.d.ts"); 2],
        ),
        (
            "pkg/subx",
            [Some("node_modules/pkg/ts56/x.d.ts"); 2],
            [Some("node_modules/pkg/ts56/x.d.ts"); 2],
        ),
    ];
    for (with_stars, files_named_star) in [(0, &[][..]), (1, STARS)] {
        for &(specifier, in_5_6, in_7) in cases {
            for (dialect, expected) in [
                (TypeScript56, in_5_6[with_stars]),
                (TypeScript7, in_7[with_stars]),
            ] {
                let config = format!(
                    r#"{{"compilerOptions":{{{BASE_OPTIONS},{PATHS}}},"files":["main.ts"]}}"#
                );
                let main = importer(&[specifier]);
                let mut files = vec![("tsconfig.json", config.as_str()), ("main.ts", &main)];
                files.extend_from_slice(&manifests);
                let labels: Vec<&str> = LABELS.iter().chain(files_named_star).copied().collect();
                let fixture = scratch(dialect, &files, &labels);
                let context = format!("{dialect:?} {specifier} (files named *: {with_stars})");
                match (fixture.try_load(), expected) {
                    (Ok(project), Some(expected)) => {
                        assert_eq!(reached(&project, 1), [expected], "{context}")
                    }
                    (Err(error), None) => {
                        assert_eq!(
                            error.kind,
                            InputErrorKind::UnresolvedImport,
                            "{context}: {error}"
                        )
                    }
                    (result, _) => panic!("{context}: {:?}", result.map(|p| reached(&p, 1))),
                }
            }
        }
    }
}

/// The documented compatibility exception for a capture's `$` sequences
/// (docs/references.md): both dialects insert a capture as written, as
/// native 7.0.2 does, in a `paths` substitution's first `*` and in every `*`
/// of an `exports` target. 5.6.3's JavaScript replacement string would
/// expand them instead: it resolves `ab$$y` to `first/$y.ts`, `ab$&x` to
/// `first/*x.ts`, and `pkg/a$&b` to `lib/a*b/a*b.d.ts` (`tsc --listFilesOnly
/// --traceResolution --incremental false`, with every file below present).
#[test]
fn a_captures_dollar_sequences_are_inserted_as_written_in_both_dialects() {
    let config = format!(
        r#"{{"compilerOptions":{{{BASE_OPTIONS},"paths":{{"ab*":["./first/*"]}}}},"files":["main.ts"]}}"#
    );
    for (specifier, expected) in [
        ("ab$$y", "first/$$y.ts"),
        ("ab$&x", "first/$&x.ts"),
        ("pkg/a$&b", "node_modules/pkg/lib/a$&b/a$&b.d.ts"),
    ] {
        for dialect in DIALECTS {
            let main = importer(&[specifier]);
            let fixture = scratch(
                dialect,
                &[
                    ("tsconfig.json", &config),
                    ("main.ts", &main),
                    (
                        "node_modules/pkg/package.json",
                        r#"{"name":"pkg","version":"1.0.0","exports":{"./*":"./lib/*/*.d.ts"}}"#,
                    ),
                ],
                &[
                    "first/$y.ts",
                    "first/$$y.ts",
                    "first/*x.ts",
                    "first/$&x.ts",
                    "node_modules/pkg/lib/a*b/a*b.d.ts",
                    "node_modules/pkg/lib/a$&b/a$&b.d.ts",
                ],
            );
            let project = fixture
                .try_load()
                .unwrap_or_else(|error| panic!("{dialect:?} {specifier}: {error}"));
            assert_eq!(reached(&project, 1), [expected], "{dialect:?} {specifier}");
        }
    }
}

/// Artemis's configuration in small, with neutral names: `baseUrl` `.` with
/// six `paths` patterns of its forms, `@/` aliases beside `testing/...`
/// bare imports that only `baseUrl` resolves, and a pattern into
/// `node_modules` that bypasses the package's own `types`.
fn artemis_shaped() -> Fixture {
    Fixture::with_dialect(
        TypeScript56,
        &[
            (
                "tsconfig.json",
                r#"{"compilerOptions":{"noLib":true,"types":[],"strict":true,"target":"ES2017","module":"ESNext","moduleResolution":"bundler","esModuleInterop":true,"jsx":"react-jsx","noEmit":true,"baseUrl":".","paths":{"@/*":["src/*"],"@checks-client/*":["checks/client/*"],"@checks-host/*":["checks/host/*"],"@checks-common/*":["checks/common/*"],"third*":["third/*"],"map-kit":["./node_modules/map-kit/dist/map-kit.d.ts"]}},"include":["src/**/*.ts","src/**/*.tsx","testing/**/*.ts","testing/**/*.tsx","checks/**/*.ts"]}"#,
            ),
            (
                "src/lib/format.ts",
                "export interface Format { pattern: string }\nexport declare function format(value: number): Format;\n",
            ),
            (
                "testing/imports.tsx",
                "export { format } from \"@/lib/format\";\nexport declare function render(name: string): number;\n",
            ),
            (
                "testing/utils.ts",
                "import { render } from \"testing/imports\";\nexport const rendered = render(\"y\");\n",
            ),
            (
                "src/feature/view.tsx",
                "import { format } from \"@/lib/format\";\nimport { render } from \"testing/imports\";\nimport { chart } from \"third/chart\";\nimport { browser } from \"@checks-client/setup\";\nimport { server } from \"@checks-host/setup\";\nexport const shown = format(render(\"x\"));\nexport const pieces = chart;\nexport const pieceB = browser;\nexport const pieceS = server;\n",
            ),
            (
                "src/feature/view.spec.tsx",
                "import { mocked } from \"@checks-common/mocks\";\nimport { format } from \"testing/imports\";\nimport { rendered } from \"testing/utils\";\nexport const viaTesting = format(rendered);\nexport const m = mocked;\n",
            ),
            (
                "checks/common/mocks.ts",
                "export declare const mocked: \"mocked\";\n",
            ),
            (
                "checks/client/setup.ts",
                "export declare const browser: \"browser\";\n",
            ),
            (
                "checks/host/setup.ts",
                "export declare const server: \"server\";\n",
            ),
            ("third/chart.ts", "export declare const chart: \"chart\";\n"),
            (
                "src/map.ts",
                "import { Map } from \"map-kit\";\nexport declare const map: Map;\nexport const zoom = map.zoom;\n",
            ),
            (
                "node_modules/map-kit/package.json",
                r#"{"name":"map-kit","version":"1.0.0","types":"dist/other.d.ts"}"#,
            ),
            (
                "node_modules/map-kit/dist/map-kit.d.ts",
                "export declare class Map { zoom: number }\n",
            ),
            (
                "node_modules/map-kit/dist/other.d.ts",
                "export declare class Map { other: string }\n",
            ),
        ],
    )
}

/// The Artemis-shaped project's references by name and types by name
/// match 5.6.3's language service (`tools/ts56-oracle.mjs`:
/// `findReferences` at each declaration, `getQuickInfoAtPosition` at each
/// name).
#[test]
fn an_artemis_shaped_project_answers_through_aliases_and_bare_imports() {
    use typescript_parser::references::ReferenceQuery;
    let fixture = artemis_shaped();
    let project = fixture.load();
    for (name, expected) in [
        ("shown", "Format"),
        ("viaTesting", "Format"),
        ("pieces", "\"chart\""),
        ("pieceB", "\"browser\""),
        ("pieceS", "\"server\""),
        ("zoom", "number"),
        ("m", "\"mocked\""),
    ] {
        let answer = project
            .type_of(TypeRequest {
                query: TypeQuery::Name {
                    name: name.into(),
                    file: None,
                },
                expand: false,
                depth: 0,
            })
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let TypeAnswer::Selected(answer) = answer else {
            panic!("{name}: {answer:?}");
        };
        assert_eq!(answer.description.summary, expected, "{name}");
    }
    for (name, expected) in [
        (
            "format",
            &[
                ("src/feature/view.spec.tsx", 2, 10),
                ("src/feature/view.spec.tsx", 4, 27),
                ("src/feature/view.tsx", 1, 10),
                ("src/feature/view.tsx", 6, 22),
                ("src/lib/format.ts", 2, 25),
                ("testing/imports.tsx", 1, 10),
            ][..],
        ),
        (
            "render",
            &[
                ("src/feature/view.tsx", 2, 10),
                ("src/feature/view.tsx", 6, 29),
                ("testing/imports.tsx", 2, 25),
                ("testing/utils.ts", 1, 10),
                ("testing/utils.ts", 2, 25),
            ][..],
        ),
    ] {
        let result = project
            .find_references(ReferenceQuery::Name {
                name: name.into(),
                file: None,
            })
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(result.groups.len(), 1, "{name}");
        let mut found: Vec<(String, usize, usize)> = result.groups[0]
            .occurrences
            .iter()
            .map(|occurrence| {
                let start = occurrence.location.start;
                (occurrence.location.file.clone(), start.line, start.column)
            })
            .collect();
        found.sort();
        let expected: Vec<(String, usize, usize)> = expected
            .iter()
            .map(|&(file, line, column)| (file.to_owned(), line, column))
            .collect();
        assert_eq!(found, expected, "{name}");
    }
}

/// The same answers through the CLI, `--direct` and `--json`, from the
/// project's directory: `--references --name` and `--type` by name and at
/// a position, through an `@/` alias and `testing/...` bare imports.
#[test]
fn the_cli_answers_the_artemis_shaped_project() {
    let fixture = artemis_shaped();
    let tsparse = |args: &[&str]| {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .args(args)
            .args(["--project", "tsconfig.json", "--direct", "--json"])
            .current_dir(&fixture.root)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success() && output.stderr.is_empty(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    for (args, expected) in [
        (&["--type", "--name", "shown"][..], "Format"),
        (&["--type", "--name", "viaTesting"][..], "Format"),
        (
            &["--type", "--at", "src/feature/view.spec.tsx:4:14"][..],
            "Format",
        ),
        (&["--type", "--name", "zoom"][..], "number"),
    ] {
        let answer = tsparse(args);
        assert_eq!(answer["outcome"], "found", "{args:?}: {answer}");
        assert_eq!(answer["type"]["summary"], expected, "{args:?}: {answer}");
    }
    // `format` through the `@/` alias and a re-export from `testing/...`;
    // `render` through `testing/...` bare imports alone.
    for (name, expected) in [
        (
            "format",
            &[
                ("src/feature/view.spec.tsx", 2, 10),
                ("src/feature/view.spec.tsx", 4, 27),
                ("src/feature/view.tsx", 1, 10),
                ("src/feature/view.tsx", 6, 22),
                ("src/lib/format.ts", 2, 25),
                ("testing/imports.tsx", 1, 10),
            ][..],
        ),
        (
            "render",
            &[
                ("src/feature/view.tsx", 2, 10),
                ("src/feature/view.tsx", 6, 29),
                ("testing/imports.tsx", 2, 25),
                ("testing/utils.ts", 1, 10),
                ("testing/utils.ts", 2, 25),
            ][..],
        ),
    ] {
        let answer = tsparse(&["--references", "--name", name]);
        assert_eq!(answer["outcome"], "complete", "{name}: {answer}");
        let mut occurrences: Vec<(String, u64, u64)> = answer["groups"][0]["occurrences"]
            .as_array()
            .unwrap()
            .iter()
            .map(|occurrence| {
                let location = &occurrence["location"];
                (
                    location["file"].as_str().unwrap().to_owned(),
                    location["start"]["line"].as_u64().unwrap(),
                    location["start"]["column"].as_u64().unwrap(),
                )
            })
            .collect();
        occurrences.sort();
        let expected: Vec<(String, u64, u64)> = expected
            .iter()
            .map(|&(file, line, column)| (file.to_owned(), line, column))
            .collect();
        assert_eq!(occurrences, expected, "{name}: {answer}");
    }
}

/// Packages whose `typesVersions` and `types@` export conditions admit only
/// `>=5.6.3 <5.7` (`tv`, `cond`) or `>=7` (`tv7`, `cond7`), beside the files
/// they would otherwise give, and files that reference both 5.6 mapped files
/// so that every program has them.
fn versioned_packages(dialect_version: &str) -> Fixture {
    let config =
        format!(r#"{{"compilerOptions":{{{BASE_OPTIONS}}},"files":["main.ts","other.ts"]}}"#);
    let main = format!(
        "/// <reference path=\"node_modules/tv/ts56/sub.d.ts\" />\n/// <reference path=\"node_modules/cond/ts56.d.ts\" />\n{}",
        importer(&["tv/sub", "cond", "tv7/sub", "cond7"])
    );
    let labels: Vec<(String, String)> = [
        "node_modules/tv/sub.d.ts",
        "node_modules/tv/ts56/sub.d.ts",
        "node_modules/cond/ts56.d.ts",
        "node_modules/cond/index.d.ts",
        "node_modules/tv7/sub.d.ts",
        "node_modules/tv7/ts7/sub.d.ts",
        "node_modules/cond7/ts7.d.ts",
        "node_modules/cond7/index.d.ts",
    ]
    .iter()
    .map(|path| ((*path).to_owned(), labelled(path)))
    .collect();
    let mut files = vec![
        ("tsconfig.json", config.as_str()),
        ("main.ts", main.as_str()),
        ("other.ts", "export const other = 1;\n"),
        (
            "node_modules/tv/package.json",
            r#"{"name":"tv","version":"1.0.0","typesVersions":{">=5.6.3 <5.7":{"*":["ts56/*"]}}}"#,
        ),
        (
            "node_modules/cond/package.json",
            r#"{"name":"cond","version":"1.0.0","exports":{".":{"types@>=5.6.3 <5.7":"./ts56.d.ts","types":"./index.d.ts"}}}"#,
        ),
        (
            "node_modules/tv7/package.json",
            r#"{"name":"tv7","version":"1.0.0","typesVersions":{">=7":{"*":["ts7/*"]}}}"#,
        ),
        (
            "node_modules/cond7/package.json",
            r#"{"name":"cond7","version":"1.0.0","exports":{".":{"types@>=7":"./ts7.d.ts","types":"./index.d.ts"}}}"#,
        ),
    ];
    files.extend(
        labels
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str())),
    );
    let fixture = Fixture::new(&files);
    fixture.claim_compiler(dialect_version);
    fixture
}

/// `typesVersions` ranges and `types@` conditions are evaluated against the
/// installed compiler's version under the 5.6 dialect, at, below and above
/// a range, and against 7.0.2 under TypeScript 7, whatever other version is
/// installed (6.0.3 selects TypeScript 7). Expected files are 5.6.3's and
/// native 7.0.2's for the same packages; where no 5.x compiler of a version
/// matches a range, its answer is the one 5.6.3 gives outside the range.
#[test]
fn types_versions_and_types_conditions_select_by_the_compiler_version() {
    const IN_5_6: [&str; 2] = [
        "node_modules/tv/ts56/sub.d.ts",
        "node_modules/cond/ts56.d.ts",
    ];
    const OUT_5_6: [&str; 2] = ["node_modules/tv/sub.d.ts", "node_modules/cond/index.d.ts"];
    const IN_7: [&str; 2] = [
        "node_modules/tv7/ts7/sub.d.ts",
        "node_modules/cond7/ts7.d.ts",
    ];
    const OUT_7: [&str; 2] = ["node_modules/tv7/sub.d.ts", "node_modules/cond7/index.d.ts"];
    for (version, [first, second]) in [
        ("5.6.3", [IN_5_6, OUT_7]),
        ("5.6.2", [OUT_5_6, OUT_7]),
        ("5.7.0", [OUT_5_6, OUT_7]),
        ("7.0.2", [OUT_5_6, IN_7]),
        ("6.0.3", [OUT_5_6, IN_7]),
    ] {
        let fixture = versioned_packages(version);
        let project = fixture.load();
        let expected: Vec<&str> = first.into_iter().chain(second).collect();
        assert_eq!(reached(&project, 4), expected, "{version}");
    }
}

/// The standard library replacement lookup (`resolveLibrary`) reads a
/// `@typescript/lib-*` package's `typesVersions` with the installed version
/// as well: with `>=5.7` before `<6`, 5.6.3 takes `v5` (`tsc --listFilesOnly`
/// lists `v5/index.d.ts`, and its quick info is `const value: number`), where
/// 7.0.2's version would take `v57`; an installed 5.7.0 takes `v57`.
#[test]
fn a_library_replacement_reads_types_versions_with_the_installed_version() {
    let Some(package) = support::typescript_5_6(
        "a_library_replacement_reads_types_versions_with_the_installed_version",
    ) else {
        return;
    };
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"types":[],"strict":true,"target":"ES2017","module":"esnext","moduleResolution":"bundler","esModuleInterop":true,"lib":["es2017","dom"]},"files":["a.ts"]}"#,
        ),
        ("a.ts", "export const value = replaced;\n"),
        (
            "node_modules/@typescript/lib-dom/package.json",
            r#"{"name":"@typescript/lib-dom","version":"1.0.0","typesVersions":{">=5.7":{"*":["v57/*"]},"<6":{"*":["v5/*"]}}}"#,
        ),
        (
            "node_modules/@typescript/lib-dom/index.d.ts",
            "declare const replaced: boolean;\n",
        ),
        (
            "node_modules/@typescript/lib-dom/v5/index.d.ts",
            "declare const replaced: number;\n",
        ),
        (
            "node_modules/@typescript/lib-dom/v57/index.d.ts",
            "declare const replaced: string;\n",
        ),
    ]);
    fixture.install_compiler(&package);
    for (version, expected) in [("5.6.3", "number"), ("5.7.0", "string")] {
        fixture.claim_compiler(version);
        let project = fixture.load();
        let answer = project
            .type_of(TypeRequest {
                query: TypeQuery::Name {
                    name: "value".into(),
                    file: None,
                },
                expand: false,
                depth: 0,
            })
            .unwrap_or_else(|error| panic!("{version}: {error}"));
        let TypeAnswer::Selected(answer) = answer else {
            panic!("{version}: {answer:?}");
        };
        assert_eq!(answer.description.summary, expected, "{version}");
    }
}

/// A `--type` request whose answer needs a fresh module spelling in a
/// `baseUrl` project: the alias to a missing export makes the checker spell
/// its module (`Module '"shared/target"' has no exported member`), and the
/// first written import of it has another resolution mode, so none is
/// reused (an import-mode name through `baseUrl` needs its extension, as
/// in 5.6.3). Spelling reaches the `baseUrl` name for a target outside the
/// project and completes from captured inputs alone, with the outcome the
/// same request has without `baseUrl` naming: the invalid alias's error
/// type, located at it. 5.6.3's `tsc`, with `"lib": ["es2022"]` in place of
/// `noLib` (without libraries it stops at TS2318), reports TS2305 there,
/// `Module '"shared/target"' has no exported member 'Missing'`.
#[test]
fn a_type_query_spells_a_base_url_module_from_captured_inputs() {
    use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
    use typescript_parser::source::SourcePosition;
    let source = "import type { present } from \"shared/target.js\" with { \"resolution-mode\": \"import\" };\nimport type { Missing } from \"shared/target\" with { \"resolution-mode\": \"require\" };\nexport type Present = typeof present;\n";
    let fixture = Fixture::with_dialect(
        TypeScript56,
        &[
            (
                "app/tsconfig.json",
                r#"{"compilerOptions":{"noLib":true,"types":[],"strict":true,"target":"es2022","module":"nodenext","moduleResolution":"nodenext","esModuleInterop":true,"baseUrl":".."},"files":["use.cts","../shared/target.d.ts"]}"#,
            ),
            ("app/use.cts", source),
            (
                "shared/target.d.ts",
                "export declare const present: number;\n",
            ),
            ("shared/index.d.ts", "export {};\n"),
            ("shared/target/index.d.ts", "export {};\n"),
        ],
    );
    let project = fixture.load_config("app/tsconfig.json");
    let at = SourcePosition {
        line: 2,
        column: 15,
    };
    let answer = project.type_of(TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("app/use.cts"),
            position: at,
        },
        expand: false,
        depth: 0,
    });
    let Err(AnalysisError::Semantic(error)) = answer else {
        panic!("{answer:?}");
    };
    assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic, "{error:?}");
    assert_eq!(
        error.reason,
        SemanticFailureReason::ErrorType.into(),
        "{error:?}"
    );
    assert_eq!(error.position, Some(at), "{error:?}");
}

//! Project navigation through the snapshot and the session: the catalog of
//! file navigation, over the project's searched files, with paths relative
//! to the project's configuration directory.
mod support;

use std::path::{Path, PathBuf};
use support::Fixture;
use typescript_parser::analysis::{
    AnalysisError, InputError, InputErrorKind, QueryError, QueryErrorKind,
};
use typescript_parser::navigation::{
    self, Candidate, Outline, Selector, Show, ShowOutcome, ShowQuery,
};
use typescript_parser::project::{Project, ProjectRequest, ProjectResponse, ProjectSession};
use typescript_parser::source::SourcePosition;

/// Checked-in navigation fixtures that load together as one project.
const FIXTURES: [&str; 13] = [
    "bindings.ts",
    "bom.ts",
    "callback-chains.ts",
    "callbacks.ts",
    "component.tsx",
    "decorators.ts",
    "expandos.ts",
    "export-equals.ts",
    "factory.ts",
    "jsdoc.ts",
    "line-breaks.ts",
    "names.ts",
    "spreads.ts",
];

const DECLARATIONS: &str = "\
declare namespace Ambient {
  interface Shape { size: number }
  function make(size: number): Shape;
}
declare function overloaded(value: string): string;
declare function overloaded(value: number): number;
";

const CONFIG: &str = r#"{ "compilerOptions": { "noLib": true, "jsx": "preserve" } }"#;

/// A project of the fixtures under `src/` and a declaration file.
fn fixture_project() -> Fixture {
    let contents: Vec<(String, String)> = FIXTURES
        .iter()
        .map(|name| {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/navigation")
                .join(name);
            (
                format!("src/{name}"),
                std::fs::read_to_string(path).unwrap(),
            )
        })
        .collect();
    let mut files: Vec<(&str, &str)> = vec![
        ("tsconfig.json", CONFIG),
        ("src/ambient.d.ts", DECLARATIONS),
    ];
    files.extend(contents.iter().map(|(a, b)| (a.as_str(), b.as_str())));
    Fixture::new(&files)
}

fn at(line: usize, column: usize) -> SourcePosition {
    SourcePosition { line, column }
}

fn outline(project: &Project, file: &Path) -> Outline {
    project
        .outline(file)
        .unwrap_or_else(|error| panic!("{error}"))
}

fn show(project: &Project, query: ShowQuery) -> Show {
    project
        .show(query.clone())
        .unwrap_or_else(|error| panic!("{query:?}: {error}"))
}

fn name(name: &str, file: Option<&Path>) -> ShowQuery {
    ShowQuery::Name {
        name: name.into(),
        file: file.map(Path::to_path_buf),
    }
}

/// An outcome with its file names removed, to compare a project answer
/// with file navigation's, whose names are absolute.
fn without_files(outcome: ShowOutcome) -> ShowOutcome {
    match outcome {
        ShowOutcome::Found(mut found) => {
            found.file.clear();
            ShowOutcome::Found(found)
        }
        ShowOutcome::Ambiguous(candidates) => ShowOutcome::Ambiguous(
            candidates
                .into_iter()
                .map(|candidate| Candidate {
                    file: String::new(),
                    ..candidate
                })
                .collect(),
        ),
        ShowOutcome::NotFound => ShowOutcome::NotFound,
    }
}

fn input_error<T: std::fmt::Debug>(result: Result<T, AnalysisError>) -> InputError {
    match result {
        Err(AnalysisError::Input(error)) => error,
        other => panic!("expected an input failure: {other:?}"),
    }
}

fn query_error<T: std::fmt::Debug>(result: Result<T, AnalysisError>) -> QueryError {
    match result {
        Err(AnalysisError::Query(error)) => error,
        other => panic!("expected a query failure: {other:?}"),
    }
}

fn config(fixture: &Fixture) -> PathBuf {
    std::fs::canonicalize(fixture.path("tsconfig.json")).unwrap()
}

#[test]
fn project_navigation_answers_as_file_navigation_for_every_searched_file() {
    let fixture = fixture_project();
    let project = fixture.load();
    let mut names: Vec<String> = FIXTURES.iter().map(|name| format!("src/{name}")).collect();
    names.push("src/ambient.d.ts".into());
    for relative in &names {
        let file = fixture.path(relative);
        let expected = navigation::outline_file(&file).unwrap();
        let actual = outline(&project, &file);
        assert_eq!(actual.project.as_deref(), Some(config(&fixture).as_path()));
        assert_eq!(&actual.file, relative);
        assert_eq!(actual.entries, expected.entries, "{relative}");
        assert!(!actual.entries.is_empty(), "{relative}");
        for entry in &actual.entries {
            let selector = Selector::At(entry.anchor);
            let in_file = navigation::show_file(&file, &selector).unwrap();
            let shown = show(
                &project,
                ShowQuery::At {
                    file: file.clone(),
                    position: entry.anchor,
                },
            );
            assert_eq!(shown.project, actual.project);
            assert_eq!(shown.file.as_deref(), Some(relative.as_str()));
            assert_eq!(shown.query, selector);
            assert_eq!(
                without_files(shown.outcome.clone()),
                without_files(in_file.outcome),
                "{relative} {:?}",
                entry.anchor
            );
            match shown.outcome {
                ShowOutcome::Found(found) => assert_eq!(&found.file, relative),
                ShowOutcome::Ambiguous(candidates) => {
                    assert!(candidates.iter().all(|c| &c.file == relative))
                }
                ShowOutcome::NotFound => panic!("{relative}: an anchor selects its entry"),
            }
            if let Some(selector) = &entry.selector {
                let in_file =
                    navigation::show_file(&file, &Selector::Name(selector.clone())).unwrap();
                let narrowed = show(&project, name(selector, Some(&file)));
                assert_eq!(
                    without_files(narrowed.outcome.clone()),
                    without_files(in_file.outcome),
                    "{relative} {selector}"
                );
                // Across the project, this file's matches are the same.
                let across = show(&project, name(selector, None));
                assert_eq!(
                    in_this_file(across.outcome, relative),
                    in_this_file(narrowed.outcome, relative),
                    "{relative} {selector}"
                );
            }
        }
    }
}

/// A selection's matches in `file`, each as a candidate.
fn in_this_file(outcome: ShowOutcome, file: &str) -> Vec<Candidate> {
    match outcome {
        ShowOutcome::Found(found) => vec![Candidate {
            file: found.file,
            label: found.declaration.label,
            kind: found.declaration.kind,
            key: found.declaration.key,
            selector: found.declaration.selector,
            anchor: found.declaration.anchor,
        }],
        ShowOutcome::Ambiguous(candidates) => candidates,
        ShowOutcome::NotFound => Vec::new(),
    }
    .into_iter()
    .filter(|candidate| candidate.file == file)
    .collect()
}

#[test]
fn only_searched_files_are_navigable() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "lib": ["es5"], "moduleResolution": "bundler", "module": "esnext" }, "include": ["src"] }"#,
        ),
        (
            "src/app.ts",
            "import { run } from \"dep\";\nexport const value = run();\n",
        ),
        (
            "node_modules/dep/package.json",
            r#"{ "name": "dep", "types": "index.d.ts" }"#,
        ),
        (
            "node_modules/dep/index.d.ts",
            "export declare function run(): number;\nexport declare function helper(): void;\n",
        ),
        ("other/outside.ts", "export const outside = 1;\n"),
    ]);
    let project = fixture.load();
    let library =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/typescript-7.0.2-lib/lib.es5.d.ts");
    for file in [
        fixture.path("node_modules/dep/index.d.ts"),
        library,
        fixture.path("other/outside.ts"),
        fixture.path("src/missing.ts"),
        PathBuf::new(),
    ] {
        for result in [
            project.outline(&file).map(|_| ()),
            project.show(name("run", Some(&file))).map(|_| ()),
            project
                .show(ShowQuery::At {
                    file: file.clone(),
                    position: at(1, 1),
                })
                .map(|_| ()),
        ] {
            let error = query_error(result);
            assert_eq!(error.kind, QueryErrorKind::OutsideProject, "{file:?}");
            assert_eq!(error.path.as_deref(), Some(file.as_path()));
        }
    }
    // The dependency's declarations are not sites of the project's names.
    let shown = show(&project, name("helper", None));
    assert_eq!(shown.outcome, ShowOutcome::NotFound);
    assert_eq!(shown.file, None);
    assert_eq!(
        outline(&project, &fixture.path("src/app.ts")).entries.len(),
        2
    );
}

#[test]
fn names_select_across_searched_files_and_candidates_round_trip_by_position() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["src"] }"#,
        ),
        (
            "src/b.ts",
            "export class Runner {\n  run() {}\n}\nexport const value = 2;\n",
        ),
        (
            "src/a.ts",
            "export function run() {}\nexport namespace Tools {\n  export function run() {}\n}\nexport const value = 1;\n",
        ),
        ("lib/c.ts", "export const value = 3;\n"),
    ]);
    let project = fixture.load();
    let base = config(&fixture);
    let directory = base.parent().unwrap();

    let shown = show(&project, name("run", None));
    assert_eq!(shown.project.as_deref(), Some(base.as_path()));
    assert_eq!(shown.file, None);
    // The result crosses a process boundary as JSON.
    let shown: Show = serde_json::from_str(&serde_json::to_string(&shown).unwrap()).unwrap();
    let ShowOutcome::Ambiguous(candidates) = shown.outcome else {
        panic!("{shown:?}");
    };
    let places: Vec<(&str, SourcePosition, &str)> = candidates
        .iter()
        .map(|c| (c.file.as_str(), c.anchor, c.selector.as_deref().unwrap()))
        .collect();
    assert_eq!(
        places,
        [
            ("src/a.ts", at(1, 17), "run"),
            ("src/a.ts", at(3, 19), "Tools.run"),
            ("src/b.ts", at(2, 3), "Runner.run"),
        ]
    );
    for candidate in &candidates {
        let again = show(
            &project,
            ShowQuery::At {
                file: directory.join(&candidate.file),
                position: candidate.anchor,
            },
        );
        let ShowOutcome::Found(found) = again.outcome else {
            panic!("{candidate:?} selected {again:?}");
        };
        assert_eq!(found.file, candidate.file);
        assert_eq!(found.declaration.anchor, candidate.anchor);
        assert_eq!(found.declaration.label, candidate.label);
        assert_eq!(found.declaration.kind, candidate.kind);
    }

    // Unsearched `lib/c.ts` is not a candidate.
    let ShowOutcome::Ambiguous(values) = show(&project, name("value", None)).outcome else {
        panic!("value is declared in two searched files");
    };
    let files: Vec<&str> = values.iter().map(|c| c.file.as_str()).collect();
    assert_eq!(files, ["src/a.ts", "src/b.ts"]);

    let ShowOutcome::Found(found) = show(&project, name("Tools.run", None)).outcome else {
        panic!("a qualified name selects one site");
    };
    assert_eq!(found.file, "src/a.ts");
    assert_eq!(found.source, "export function run() {}");
    assert_eq!(found.declaration.range.start, at(3, 3));

    let narrowed = show(&project, name("run", Some(&fixture.path("src/b.ts"))));
    assert_eq!(narrowed.file.as_deref(), Some("src/b.ts"));
    let ShowOutcome::Found(found) = narrowed.outcome else {
        panic!("{narrowed:?}");
    };
    assert_eq!(
        (found.file.as_str(), found.source.as_str()),
        ("src/b.ts", "run() {}")
    );

    let missing = show(&project, name("absent", None));
    assert_eq!(missing.outcome, ShowOutcome::NotFound);
    assert_eq!(missing.query, Selector::Name("absent".into()));
    assert_eq!(
        show(&project, name("value", Some(&fixture.path("src/b.ts")))).file,
        Some("src/b.ts".into())
    );
}

/// Dotted namespaces, a namespace nested in another, ambient modules, and
/// a global augmentation: the keys whose segments the selection index
/// stores joined, and containers named by strings or `global`.
fn namespace_project() -> Fixture {
    Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "src/dotted.ts",
            "export namespace A.B.C {\n  export const k = 1;\n}\ndeclare global {\n  interface Shared { k: number }\n}\n",
        ),
        (
            "src/nested.ts",
            "export namespace A.B {\n  export namespace C {\n    export const k = 2;\n  }\n}\n",
        ),
        (
            "src/ambient.d.ts",
            "declare module \"m\" {\n  export const k: number;\n  export namespace A.B.C {\n    const k: number;\n  }\n}\ndeclare module \"a.b\" {\n  export const k: number;\n}\n",
        ),
    ])
}

/// Each candidate's file, selector, and anchor.
fn places(outcome: ShowOutcome) -> Vec<(String, Option<String>, SourcePosition)> {
    let ShowOutcome::Ambiguous(candidates) = outcome else {
        panic!("expected candidates: {outcome:?}");
    };
    candidates
        .into_iter()
        .map(|candidate| (candidate.file, candidate.selector, candidate.anchor))
        .collect()
}

fn place(
    file: &str,
    selector: Option<&str>,
    position: SourcePosition,
) -> (String, Option<String>, SourcePosition) {
    (file.into(), selector.map(Into::into), position)
}

#[test]
fn qualified_names_follow_dotted_namespaces_ambient_modules_and_global_augmentations() {
    let fixture = namespace_project();
    let project = fixture.load();
    let (ambient, dotted, nested) = ("src/ambient.d.ts", "src/dotted.ts", "src/nested.ts");
    assert_eq!(
        places(show(&project, name("k", None)).outcome),
        [
            place(ambient, Some("m.k"), at(2, 16)),
            place(ambient, Some("m.A.B.C.k"), at(4, 11)),
            // `"a.b"` is one key the selector language cannot spell.
            place(ambient, None, at(8, 16)),
            place(dotted, Some("A.B.C.k"), at(2, 16)),
            place(dotted, Some("global.Shared.k"), at(5, 22)),
            place(nested, Some("A.B.C.k"), at(3, 18)),
        ]
    );
    // Every suffix of a dotted chain selects the one dotted site, and the
    // nested form's innermost namespace.
    for selector in ["C", "B.C", "A.B.C"] {
        assert_eq!(
            places(show(&project, name(selector, None)).outcome),
            [
                place(ambient, Some("m.A.B.C"), at(3, 20)),
                place(dotted, Some("A.B.C"), at(1, 18)),
                place(nested, Some("A.B.C"), at(2, 20)),
            ],
            "{selector}"
        );
    }
    for selector in ["C.k", "B.C.k", "A.B.C.k"] {
        assert_eq!(
            places(show(&project, name(selector, None)).outcome),
            [
                place(ambient, Some("m.A.B.C.k"), at(4, 11)),
                place(dotted, Some("A.B.C.k"), at(2, 16)),
                place(nested, Some("A.B.C.k"), at(3, 18)),
            ],
            "{selector}"
        );
    }
    // Segments of a dotted key match only in order and in full.
    let outer = "export namespace A.B {\n  export namespace C {\n    export const k = 2;\n  }\n}";
    for (selector, file, source) in [
        ("A.B", nested, outer),
        ("B", nested, outer),
        ("m.k", ambient, "export const k: number;"),
        ("m.A.B.C.k", ambient, "const k: number;"),
        ("global.Shared.k", dotted, "k: number"),
        ("global.Shared", dotted, "interface Shared { k: number }"),
    ] {
        let ShowOutcome::Found(found) = show(&project, name(selector, None)).outcome else {
            panic!("{selector} selects one site");
        };
        assert_eq!(
            (found.file.as_str(), found.source.as_str()),
            (file, source),
            "{selector}"
        );
        assert!(
            found
                .declaration
                .selector
                .as_deref()
                .unwrap()
                .ends_with(selector),
            "{selector}"
        );
    }
    for selector in ["a.b.k", "A", "A.C", "A.B.C.C", "m.C.k"] {
        assert_eq!(
            show(&project, name(selector, None)).outcome,
            ShowOutcome::NotFound,
            "{selector}"
        );
    }
    // Narrowed to a file, a name follows the same rule.
    let narrowed = show(&project, name("k", Some(&fixture.path(dotted))));
    assert_eq!(narrowed.file.as_deref(), Some(dotted));
    assert_eq!(
        places(narrowed.outcome),
        [
            place(dotted, Some("A.B.C.k"), at(2, 16)),
            place(dotted, Some("global.Shared.k"), at(5, 22)),
        ]
    );
    let ShowOutcome::Found(found) =
        show(&project, name("A.B.C.k", Some(&fixture.path(nested)))).outcome
    else {
        panic!("one A.B.C.k in nested.ts");
    };
    assert_eq!(found.declaration.anchor, at(3, 18));
    // Outlines carry the same keys and selectors.
    let entries = outline(&project, &fixture.path(dotted)).entries;
    let described: Vec<(&str, Option<&str>, Option<&str>)> = entries
        .iter()
        .map(|entry| {
            (
                entry.label.as_str(),
                entry.key.as_deref(),
                entry.selector.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        described,
        [
            ("A.B.C", Some("A.B.C"), Some("A.B.C")),
            ("k", Some("k"), Some("A.B.C.k")),
            ("global", Some("global"), Some("global")),
            ("Shared", Some("Shared"), Some("global.Shared")),
            ("k", Some("k"), Some("global.Shared.k")),
        ]
    );
    let entries = outline(&project, &fixture.path(ambient)).entries;
    let described: Vec<(&str, Option<&str>, Option<&str>)> = entries
        .iter()
        .map(|entry| {
            (
                entry.label.as_str(),
                entry.key.as_deref(),
                entry.selector.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        described,
        [
            ("\"m\"", Some("m"), Some("m")),
            ("k", Some("k"), Some("m.k")),
            ("A.B.C", Some("A.B.C"), Some("m.A.B.C")),
            ("k", Some("k"), Some("m.A.B.C.k")),
            ("\"a.b\"", Some("a.b"), None),
            ("k", Some("k"), None),
        ]
    );
}

/// Every query naming `file`, through the one request dispatch.
fn every_query(file: &Path) -> Vec<ProjectRequest> {
    use typescript_parser::references::ReferenceQuery;
    let file = file.to_path_buf();
    vec![
        ProjectRequest::Outline { file: file.clone() },
        ProjectRequest::Show(name("value", Some(&file))),
        ProjectRequest::Show(ShowQuery::At {
            file: file.clone(),
            position: at(1, 14),
        }),
        ProjectRequest::References(ReferenceQuery::At {
            file: file.clone(),
            position: at(1, 14),
        }),
        ProjectRequest::References(ReferenceQuery::Name {
            name: "value".into(),
            file: Some(file),
        }),
    ]
}

#[cfg(unix)]
#[test]
fn query_paths_are_names_that_must_be_exact() {
    use std::os::unix::ffi::OsStringExt;
    // A real file named with U+FFFD, which a lossy conversion of a name
    // that is not Unicode produces. Such a query path is unsupported, never
    // that file; an exact name of no searched file is outside the project.
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        ("bad\u{FFFD}.ts", "export const value = 1;\n"),
    ]);
    let project = fixture.load();
    for request in every_query(&fixture.path("bad\u{FFFD}.ts")) {
        assert!(project.execute(request.clone()).is_ok(), "{request:?}");
    }
    for spelling in [&b"bad\xff.ts"[..], b"bad\xc3.ts"] {
        let mut bytes = fixture.root.clone().into_os_string().into_vec();
        bytes.push(b'/');
        bytes.extend_from_slice(spelling);
        let file = PathBuf::from(std::ffi::OsString::from_vec(bytes));
        assert_eq!(
            file.to_string_lossy(),
            fixture.path("bad\u{FFFD}.ts").to_str().unwrap()
        );
        for request in every_query(&file) {
            let error = input_error(project.execute(request.clone()));
            assert_eq!(error.kind, InputErrorKind::Unsupported, "{request:?}");
            assert_eq!(
                error.message,
                "the file name is not Unicode, so it cannot be named exactly"
            );
            assert_eq!(error.path.as_deref(), Some(file.as_path()));
            assert_eq!(error.position, None);
        }
    }
    for file in [
        fixture.path("bad\u{FFFD}\u{FFFD}.ts"),
        fixture.path("bad.ts"),
    ] {
        for request in every_query(&file) {
            let error = query_error(project.execute(request.clone()));
            assert_eq!(error.kind, QueryErrorKind::OutsideProject, "{request:?}");
            assert_eq!(error.path.as_deref(), Some(file.as_path()));
        }
    }
}

#[test]
fn positions_select_names_and_anchors_and_invalid_positions_fail() {
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        ("src/astral.ts", "const \u{1D465}y = 1;\nfunction f() {}\n"),
    ]);
    let project = fixture.load();
    let file = fixture.path("src/astral.ts");
    let found = |position| match show(
        &project,
        ShowQuery::At {
            file: file.clone(),
            position,
        },
    )
    .outcome
    {
        ShowOutcome::Found(found) => Some(found.declaration.label),
        ShowOutcome::NotFound => None,
        other => panic!("{other:?}"),
    };
    assert_eq!(found(at(1, 9)).as_deref(), Some("\u{1D465}y"));
    assert_eq!(found(at(2, 10)).as_deref(), Some("f"));
    // Inside a declaration but not on its name, past a name, and line ends.
    for position in [at(1, 1), at(1, 10), at(2, 16), at(3, 1)] {
        assert_eq!(found(position), None, "{position:?}");
    }
    for (position, message) in [
        (at(0, 1), "line is outside the file"),
        (at(4, 1), "line is outside the file"),
        (at(2, 17), "column is outside the line"),
        (at(1, 8), "column is inside a surrogate pair"),
    ] {
        let error = query_error(project.show(ShowQuery::At {
            file: file.clone(),
            position,
        }));
        assert_eq!(error.kind, QueryErrorKind::InvalidPosition);
        assert_eq!(error.message, message);
        assert_eq!(error.position, Some(position));
        assert_eq!(error.path.as_deref(), Some(file.as_path()));
    }
}

#[test]
fn project_navigation_parses_with_the_deep_budget() {
    // Modules, so the files' outer `f`s do not conflict. As in file
    // navigation, 204 nested functions are the most the budget accepts.
    let nested = |count: usize| {
        format!(
            "{}{}\nexport {{}};\n",
            "function f() {".repeat(count),
            "}".repeat(count)
        )
    };
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        ("under.ts", &nested(203)),
        ("at.ts", &nested(204)),
    ]);
    let project = fixture.load();
    for (file, depth) in [("under.ts", 203), ("at.ts", 204)] {
        let entries = outline(&project, &fixture.path(file)).entries;
        assert_eq!(entries.len(), depth);
        let deepest = entries.last().unwrap();
        assert_eq!(deepest.depth, depth);
        let selector = vec!["f"; depth].join(".");
        assert_eq!(deepest.selector.as_deref(), Some(selector.as_str()));
        // Only the deepest `f` of its file has as many containers.
        let shown = show(&project, name(&selector, Some(&fixture.path(file))));
        let ShowOutcome::Found(found) = shown.outcome else {
            panic!("{file}");
        };
        assert_eq!(found.declaration.anchor, deepest.anchor);
    }
    let over = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        ("over.ts", &nested(205)),
    ]);
    let error = Project::load(&over.path("tsconfig.json")).unwrap_err();
    assert_eq!(error.kind, InputErrorKind::ResourceLimit);
    assert_eq!(error.message, "Maximum syntax nesting depth exceeded");
}

#[test]
fn long_chains_and_flat_lists_are_navigable_in_full() {
    // A member chain parses in a loop, so no grammar budget bounds it; a
    // callback's label names the whole chain, never truncated (TypeScript
    // cuts it at 150 characters). Flat lists of callbacks and of
    // declarations stay shallow.
    let chain = format!("chain{}", ".link".repeat(2_000));
    let calls = "list.add(() => {});\n".repeat(5_000);
    let constants: String = (0..5_000)
        .map(|index| format!("export const item{index} = {index};\n"))
        .collect();
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        (
            "chain.ts",
            &format!(
                "declare const chain: any;\n{chain}(() => {{\n  const inside = 1;\n}});\nexport {{}};\n"
            ),
        ),
        (
            "flat.ts",
            &format!("declare const list: any;\n{calls}{constants}"),
        ),
    ]);
    let project = fixture.load();
    let label = format!("{chain}() callback");
    let entries = outline(&project, &fixture.path("chain.ts")).entries;
    let labels: Vec<&str> = entries.iter().map(|entry| entry.label.as_str()).collect();
    assert_eq!(labels, ["chain", label.as_str(), "inside"]);
    let anchor = entries[1].anchor;
    assert_eq!(anchor, at(2, chain.len() + 2));
    let ShowOutcome::Found(found) = show(
        &project,
        ShowQuery::At {
            file: fixture.path("chain.ts"),
            position: anchor,
        },
    )
    .outcome
    else {
        panic!("the callback is anchored at its first token");
    };
    assert_eq!(found.declaration.label, label);
    // The callback has no key, so its local's selector skips it.
    let ShowOutcome::Found(found) = show(&project, name("inside", None)).outcome else {
        panic!("one inside");
    };
    assert_eq!(found.declaration.selector.as_deref(), Some("inside"));

    let entries = outline(&project, &fixture.path("flat.ts")).entries;
    assert_eq!(entries.len(), 1 + 5_000 + 5_000);
    assert!(
        entries[1..=5_000]
            .iter()
            .all(|entry| entry.label == "list.add() callback" && entry.depth == 1)
    );
    assert_eq!(entries.last().unwrap().label, "item4999");
    let ShowOutcome::Found(found) = show(&project, name("item4999", None)).outcome else {
        panic!("one item4999");
    };
    assert_eq!(found.source, "export const item4999 = 4999;");
}

#[test]
fn json_modules_in_the_project_are_not_navigation_input() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true, "resolveJsonModule": true, "module": "esnext", "moduleResolution": "bundler" }, "include": ["src"] }"#,
        ),
        (
            "src/app.ts",
            "import data from \"./data.json\";\nexport const size = data.size;\n",
        ),
        ("src/data.json", "{ \"size\": 1 }\n"),
    ]);
    let project = fixture.load();
    let json = fixture.path("src/data.json");
    for result in [
        project.outline(&json).map(|_| ()),
        project.show(name("size", Some(&json))).map(|_| ()),
        project
            .show(ShowQuery::At {
                file: json.clone(),
                position: at(1, 3),
            })
            .map(|_| ()),
    ] {
        let error = input_error(result);
        assert_eq!(error.kind, InputErrorKind::Unsupported);
        assert_eq!(error.message, "JSON files are not navigation input");
        assert_eq!(error.path.as_deref(), Some(json.as_path()));
    }
    // A name search covers the TypeScript files and skips the JSON module.
    let ShowOutcome::Found(found) = show(&project, name("size", None)).outcome else {
        panic!("size is declared once in TypeScript");
    };
    assert_eq!(found.file, "src/app.ts");
}

fn session_outline(session: &mut ProjectSession, file: &Path) -> Result<Outline, AnalysisError> {
    session
        .execute(ProjectRequest::Outline {
            file: file.to_path_buf(),
        })
        .map(|response| match response {
            ProjectResponse::Outline(outline) => outline,
            other => panic!("an outline request answered {other:?}"),
        })
}

fn session_show(session: &mut ProjectSession, query: ShowQuery) -> Result<Show, AnalysisError> {
    session
        .execute(ProjectRequest::Show(query))
        .map(|response| match response {
            ProjectResponse::Show(show) => *show,
            other => panic!("a show request answered {other:?}"),
        })
}

fn labels(outline: &Outline) -> Vec<&str> {
    outline.entries.iter().map(|e| e.label.as_str()).collect()
}

#[test]
fn sessions_answer_navigation_from_their_current_generation() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["src"] }"#,
        ),
        ("src/a.ts", "export function before() {}\n"),
        ("other/b.ts", "export function elsewhere() {}\n"),
    ]);
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    let a = fixture.path("src/a.ts");
    let found_label =
        |session: &mut ProjectSession, query| match session_show(session, query).unwrap().outcome {
            ShowOutcome::Found(found) => Some(found.declaration.label),
            ShowOutcome::NotFound => None,
            other => panic!("{other:?}"),
        };
    assert_eq!(
        labels(&session_outline(&mut session, &a).unwrap()),
        ["before"]
    );
    assert_eq!(
        found_label(&mut session, name("before", None)).as_deref(),
        Some("before")
    );
    assert_eq!(
        session.generation(),
        1,
        "unchanged requests reuse the generation"
    );

    // A changed source is answered from a new generation by every operation.
    std::fs::write(&a, "export function after() {}\n").unwrap();
    assert_eq!(
        labels(&session_outline(&mut session, &a).unwrap()),
        ["after"]
    );
    assert_eq!(found_label(&mut session, name("before", None)), None);
    assert_eq!(
        found_label(
            &mut session,
            ShowQuery::At {
                file: a.clone(),
                position: at(1, 17)
            }
        )
        .as_deref(),
        Some("after")
    );
    // References answer from the same generation.
    let after = typescript_parser::references::ReferenceQuery::Name {
        name: "after".into(),
        file: None,
    };
    let groups = support::references(&mut session, after).unwrap().groups;
    assert_eq!(groups.len(), 1);
    assert_eq!(session.generation(), 2);

    // A configuration change moves the searched files.
    let b = fixture.path("other/b.ts");
    assert_eq!(
        query_error(session_outline(&mut session, &b)).kind,
        QueryErrorKind::OutsideProject
    );
    std::fs::write(
        fixture.path("tsconfig.json"),
        r#"{ "compilerOptions": { "noLib": true }, "include": ["src", "other"] }"#,
    )
    .unwrap();
    assert_eq!(
        labels(&session_outline(&mut session, &b).unwrap()),
        ["elsewhere"]
    );
    assert_eq!(
        found_label(&mut session, name("elsewhere", None)).as_deref(),
        Some("elsewhere")
    );
    assert_eq!(session.generation(), 3);

    // A failed refresh answers neither operation from the old generation,
    // nor falls back to navigating the file alone.
    std::fs::write(fixture.path("other/b.ts"), "export function = ;\n").unwrap();
    for _ in 0..2 {
        let error = input_error(session_outline(&mut session, &a));
        assert_eq!(error.kind, InputErrorKind::Syntax);
        assert_eq!(error.path.as_deref(), Some(b.as_path()));
        let error = input_error(session_show(&mut session, name("after", Some(&a))));
        assert_eq!(error.kind, InputErrorKind::Syntax);
        let error = input_error(session_show(
            &mut session,
            ShowQuery::At {
                file: a.clone(),
                position: at(1, 17),
            },
        ));
        assert_eq!(error.kind, InputErrorKind::Syntax);
    }
    assert_eq!(navigation::outline_file(&a).unwrap().entries.len(), 1);
    std::fs::write(
        fixture.path("other/b.ts"),
        "export function repaired() {}\n",
    )
    .unwrap();
    assert_eq!(
        labels(&session_outline(&mut session, &b).unwrap()),
        ["repaired"]
    );
    assert_eq!(session.generation(), 4);
}

#[test]
fn the_snapshot_dispatches_every_request_without_refreshing() {
    let fixture = Fixture::new(&[
        ("tsconfig.json", support::CONFIG),
        ("a.ts", "export function captured() {}\n"),
    ]);
    let project = fixture.load();
    let a = fixture.path("a.ts");
    std::fs::write(&a, "export function edited() {}\n").unwrap();
    let ProjectResponse::Outline(outline) = project
        .execute(ProjectRequest::Outline { file: a.clone() })
        .unwrap()
    else {
        panic!("an outline request answers an outline");
    };
    assert_eq!(labels(&outline), ["captured"]);
    let ProjectResponse::Show(shown) = project
        .execute(ProjectRequest::Show(name("captured", None)))
        .unwrap()
    else {
        panic!("a show request answers a selection");
    };
    let ShowOutcome::Found(found) = shown.outcome else {
        panic!("the captured text is shown");
    };
    assert_eq!(found.source, "export function captured() {}");
    assert!(matches!(
        project.execute(ProjectRequest::References(
            typescript_parser::references::ReferenceQuery::Name {
                name: "captured".into(),
                file: None
            }
        )),
        Ok(ProjectResponse::References(result)) if result.groups.len() == 1
    ));
}

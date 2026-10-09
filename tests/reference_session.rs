//! Long-lived callers reuse a generation but observe completed filesystem edits.
mod support;

use support::project;
use typescript_parser::analysis::{AnalysisError, InputErrorKind};
use typescript_parser::project::ProjectSession;
use typescript_parser::references::ReferenceQuery;

fn name(name: &str) -> ReferenceQuery {
    ReferenceQuery::Name {
        name: name.into(),
        file: None,
    }
}

fn count(session: &mut ProjectSession, text: &str) -> usize {
    support::references(session, name(text))
        .unwrap()
        .groups
        .len()
}

#[test]
fn repeated_queries_reuse_one_complete_generation() {
    let fixture = project(&[("a.ts", "export const value = 1; value;\n")]);
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    let query = || ReferenceQuery::Name {
        name: "value".into(),
        file: None,
    };
    let first = support::references(&mut session, query()).unwrap();
    assert_eq!(first.groups.len(), 1);
    assert_eq!(first.groups[0].occurrences.len(), 2);
    assert_eq!(session.generation(), 1);
    assert_eq!(support::references(&mut session, query()).unwrap(), first);
    assert_eq!(
        session.generation(),
        1,
        "unchanged queries must reuse the index"
    );
}

#[test]
fn same_size_and_timestamp_edits_refresh_and_failed_refreshes_do_not_serve_stale_results() {
    let fixture = project(&[("a.ts", "export const before = 1; before;\n")]);
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    assert_eq!(count(&mut session, "before"), 1);
    let path = fixture.path("a.ts");
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::fs::write(&path, "export const after_ = 1; after_;\n").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert_eq!(count(&mut session, "before"), 0);
    assert_eq!(count(&mut session, "after_"), 1);
    assert_eq!(session.generation(), 2);

    std::fs::write(&path, "export const = ;").unwrap();
    for _ in 0..2 {
        assert!(matches!(support::references(&mut session, name("after_")),
            Err(AnalysisError::Input(error)) if error.kind == InputErrorKind::Syntax));
    }
    std::fs::write(&path, "export const repaired = 1;").unwrap();
    assert_eq!(count(&mut session, "repaired"), 1);
    assert_eq!(count(&mut session, "after_"), 0);
    assert_eq!(session.generation(), 3);
}

#[test]
fn wildcard_additions_deletions_and_inherited_configuration_refresh() {
    let fixture = project(&[
        ("tsconfig.json", r#"{"extends":"./base.json"}"#),
        (
            "base.json",
            r#"{"compilerOptions":{"noLib":true},"include":["src"]}"#,
        ),
        ("src/a.ts", "export const value = 1;"),
        ("other/b.ts", "export const elsewhere = 1;"),
    ]);
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    assert_eq!(count(&mut session, "value"), 1);
    std::fs::write(fixture.path("src/new.ts"), "export const value = 2;").unwrap();
    assert_eq!(count(&mut session, "value"), 2);
    std::fs::remove_file(fixture.path("src/a.ts")).unwrap();
    assert_eq!(count(&mut session, "value"), 1);
    std::fs::write(
        fixture.path("base.json"),
        r#"{"compilerOptions":{"noLib":true},"include":["other"]}"#,
    )
    .unwrap();
    assert_eq!(count(&mut session, "value"), 0);
    assert_eq!(count(&mut session, "elsewhere"), 1);
    assert_eq!(session.generation(), 4);
}

#[test]
fn creation_of_previously_absent_higher_priority_import_target_refreshes() {
    // Explicit roots prevent a directory scan from hiding missing probe tracking.
    let fixture = project(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"noLib":true,"moduleResolution":"bundler","module":"esnext"},"files":["app.ts"]}"#,
        ),
        ("app.ts", "import { value } from './dep'; value;"),
        ("dep.d.ts", "export declare const value: number;"),
    ]);
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    let declaration = |session: &mut ProjectSession| {
        support::references(session, name("value")).unwrap().groups[0].declarations[0]
            .location
            .file
            .clone()
    };
    assert_eq!(declaration(&mut session), "dep.d.ts");
    std::fs::write(fixture.path("dep.ts"), "export const value = 1;").unwrap();
    assert_eq!(declaration(&mut session), "dep.ts");
    std::fs::remove_file(fixture.path("dep.ts")).unwrap();
    assert_eq!(declaration(&mut session), "dep.d.ts");
    assert_eq!(session.generation(), 3);
}

#[test]
fn package_manifest_and_dependency_contents_are_inputs() {
    let fixture = project(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"noLib":true,"moduleResolution":"bundler","module":"esnext"},"files":["app.ts"]}"#,
        ),
        ("app.ts", "import { value } from 'pkg'; value;"),
        ("node_modules/pkg/package.json", r#"{"types":"a.d.ts"}"#),
        (
            "node_modules/pkg/a.d.ts",
            "export declare const value: number;",
        ),
        (
            "node_modules/pkg/b.d.ts",
            "export declare const value: string;",
        ),
    ]);
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    let declaration = |session: &mut ProjectSession| {
        support::references(session, name("value")).unwrap().groups[0].declarations[0]
            .location
            .clone()
    };
    assert!(declaration(&mut session).file.ends_with("a.d.ts"));
    std::fs::write(
        fixture.path("node_modules/pkg/package.json"),
        r#"{"types":"b.d.ts"}"#,
    )
    .unwrap();
    assert!(declaration(&mut session).file.ends_with("b.d.ts"));
    std::fs::write(
        fixture.path("node_modules/pkg/b.d.ts"),
        "\nexport declare const value: string;",
    )
    .unwrap();
    assert_eq!(declaration(&mut session).start.line, 2);
    assert_eq!(session.generation(), 3);
}

#[cfg(unix)]
#[test]
fn symlink_retargeting_refreshes_even_when_previous_target_is_unchanged() {
    let fixture = project(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"noLib":true},"files":["link.ts"]}"#,
        ),
        ("a.ts", "export const first = 1;"),
        ("b.ts", "export const second = 1;"),
    ]);
    std::os::unix::fs::symlink(fixture.path("a.ts"), fixture.path("link.ts")).unwrap();
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    assert_eq!(count(&mut session, "first"), 1);
    std::fs::remove_file(fixture.path("link.ts")).unwrap();
    std::os::unix::fs::symlink(fixture.path("b.ts"), fixture.path("link.ts")).unwrap();
    assert_eq!(count(&mut session, "first"), 0);
    assert_eq!(count(&mut session, "second"), 1);
    assert_eq!(session.generation(), 2);
}

#[test]
fn captured_and_refreshed_queries_preserve_every_pinned_semantic_fixture() {
    let mut fixtures: Vec<_> = std::fs::read_dir(support::fixtures_root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("tsconfig.json").is_file())
        .collect();
    fixtures.sort();
    for path in fixtures {
        let fixture = support::Fixture::existing(&path);
        let project = fixture.load();
        fixture.check_markers(&project);
        let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
        for marker in fixture.markers() {
            let query = ReferenceQuery::At {
                file: fixture.path(&marker.location.file),
                position: marker.location.start,
            };
            match (
                project.find_references(query.clone()),
                support::references(&mut session, query),
            ) {
                (Ok(expected), Ok(actual)) => assert_eq!(actual, expected),
                (Err(expected), Err(actual)) => assert_eq!(actual, expected),
                pair => panic!("different outcomes in {}: {pair:?}", path.display()),
            }
        }
        assert_eq!(session.generation(), 1, "{}", path.display());
    }
}

#[test]
fn unreadable_input_remains_an_io_failure_and_recovers() {
    let fixture = project(&[("a.ts", "export const value = 1;")]);
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    assert_eq!(count(&mut session, "value"), 1);
    let config = fixture.path("tsconfig.json");
    std::fs::remove_file(&config).unwrap();
    std::fs::create_dir(&config).unwrap();
    assert!(
        matches!(support::references(&mut session, name("value")), Err(AnalysisError::Input(error)) if error.kind == InputErrorKind::Io)
    );
    std::fs::remove_dir(&config).unwrap();
    std::fs::write(&config, support::CONFIG).unwrap();
    assert_eq!(count(&mut session, "value"), 1);
    assert_eq!(session.generation(), 2);
}

#[test]
fn an_old_unreadable_input_is_not_an_error_when_current_configuration_no_longer_selects_it() {
    let fixture = project(&[
        ("a.ts", "export const before = 1;"),
        ("b.ts", "export const after = 1;"),
    ]);
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    assert_eq!(count(&mut session, "before"), 1);
    std::fs::remove_file(fixture.path("a.ts")).unwrap();
    std::fs::create_dir(fixture.path("a.ts")).unwrap();
    std::fs::write(
        fixture.path("tsconfig.json"),
        r#"{"compilerOptions":{"noLib":true},"files":["b.ts"]}"#,
    )
    .unwrap();
    assert!(
        fixture
            .load()
            .find_references(name("before"))
            .unwrap()
            .groups
            .is_empty()
    );
    let result = support::references(&mut session, name("after"));
    assert!(
        result.is_ok(),
        "obsolete inputs must not prevent loading the current project: {result:?}"
    );
    assert_eq!(result.unwrap().groups.len(), 1);
    assert_eq!(session.generation(), 2);
}

#[test]
fn a_previously_missing_or_non_directory_include_root_can_appear() {
    for was_file in [false, true] {
        let fixture = project(&[
            (
                "tsconfig.json",
                r#"{"compilerOptions":{"noLib":true},"files":["a.ts"],"include":["future/**/*"]}"#,
            ),
            ("a.ts", "export const value = 1;"),
        ]);
        if was_file {
            std::fs::write(fixture.path("future"), "not a directory").unwrap();
        }
        let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
        assert_eq!(count(&mut session, "value"), 1);
        if was_file {
            std::fs::remove_file(fixture.path("future")).unwrap();
        }
        std::fs::create_dir(fixture.path("future")).unwrap();
        std::fs::write(fixture.path("future/b.ts"), "export const value = 2;").unwrap();
        assert_eq!(count(&mut session, "value"), 2);
        assert_eq!(session.generation(), 2);
    }
}

#[test]
fn new_automatic_type_package_is_observed_without_root_source_edits() {
    let fixture = project(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"noLib":true,"typeRoots":["./typings"],"types":["*"]},"files":["app.ts"]}"#,
        ),
        ("app.ts", "newGlobal;"),
    ]);
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    let query = || ReferenceQuery::At {
        file: fixture.path("app.ts"),
        position: typescript_parser::source::SourcePosition { line: 1, column: 1 },
    };
    assert!(
        matches!(support::references(&mut session, query()), Err(AnalysisError::Query(error)) if error.kind == typescript_parser::analysis::QueryErrorKind::UnresolvedTarget)
    );
    std::fs::create_dir_all(fixture.path("typings/added")).unwrap();
    std::fs::write(
        fixture.path("typings/added/index.d.ts"),
        "declare const newGlobal: number;",
    )
    .unwrap();
    let result = support::references(&mut session, query()).unwrap();
    assert_eq!(result.groups.len(), 1);
    assert_eq!(
        result.groups[0].declarations[0].location.file,
        "typings/added/index.d.ts"
    );
    assert_eq!(session.generation(), 2);
}

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

/// Validation compares every captured observation, wherever its path sorts
/// among many: contents, directory listings, resolution probes, and real
/// paths, for the first and last inputs as well as one in the middle.
#[cfg(unix)]
#[test]
fn every_kind_of_input_is_validated_wherever_it_sorts_in_a_large_project() {
    let mut files = vec![(
        "tsconfig.json".to_owned(),
        r#"{"compilerOptions":{"noLib":true},"include":["d*/**/*","zlink.ts"]}"#.to_owned(),
    )];
    for directory in 0..24 {
        for file in 0..24 {
            files.push((
                format!("d{directory:02}/m{file:02}.ts"),
                format!(
                    "import {{ shared{file} }} from '../ydeps/dep{file}';\nexport const v{directory}_{file} = shared{file};\n"
                ),
            ));
        }
    }
    for file in 0..24 {
        files.push((
            format!("ydeps/dep{file}.d.ts"),
            format!("export declare const shared{file}: number;\n"),
        ));
    }
    files.push(("a.ts".to_owned(), "export const linkedA = 1;".to_owned()));
    files.push(("b.ts".to_owned(), "export const linkedB = 1;".to_owned()));
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let fixture = support::Fixture::new(&borrowed);
    std::os::unix::fs::symlink(fixture.path("a.ts"), fixture.path("zlink.ts")).unwrap();
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    assert_eq!(count(&mut session, "v23_23"), 1);
    assert_eq!(count(&mut session, "v0_0"), 1);
    assert_eq!(
        session.generation(),
        1,
        "an unchanged project reuses its generation"
    );

    // Same-size edits that restore the modification time, at both ends of
    // the key order and in the middle.
    for (directory, file) in [(23, 23), (0, 0), (11, 12)] {
        let path = fixture.path(&format!("d{directory:02}/m{file:02}.ts"));
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let before = std::fs::read_to_string(&path).unwrap();
        let old = format!("v{directory}_{file}");
        let new = format!("w{directory}_{file}");
        std::fs::write(&path, before.replace(&old, &new)).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(count(&mut session, &old), 0, "{old}");
        assert_eq!(count(&mut session, &new), 1, "{new}");
    }
    assert_eq!(session.generation(), 4);

    // A listing: a new file in the directory that sorts last.
    std::fs::write(fixture.path("d23/added.ts"), "export const added = 1;").unwrap();
    assert_eq!(count(&mut session, "added"), 1);
    assert_eq!(session.generation(), 5);

    // A resolution probe outside every listed directory: a previously
    // absent, higher-priority import target.
    std::fs::write(
        fixture.path("ydeps/dep23.ts"),
        "export const shared23 = 1;\nexport const replaced = 1;\n",
    )
    .unwrap();
    assert_eq!(count(&mut session, "replaced"), 1);
    assert_eq!(session.generation(), 6);

    // A real path: the symbolic link that sorts last changes its target.
    std::fs::remove_file(fixture.path("zlink.ts")).unwrap();
    std::os::unix::fs::symlink(fixture.path("b.ts"), fixture.path("zlink.ts")).unwrap();
    assert_eq!(count(&mut session, "linkedA"), 0);
    assert_eq!(count(&mut session, "linkedB"), 1);
    assert_eq!(session.generation(), 7);
    assert_eq!(count(&mut session, "linkedB"), 1);
    assert_eq!(session.generation(), 7);
}

/// An input that changes after loading read it, while loading still runs,
/// fails the refresh rather than publishing what was read: the check after
/// loading compares every observation again. Here a pipe whose writer
/// answers a second late keeps loading running while an input already read
/// changes; the pipe sorts after it, so the check never waits on the pipe.
#[cfg(unix)]
#[test]
fn an_input_changed_while_loading_fails_the_refresh() {
    let fixture = project(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"noLib":true},"files":["a.ts","first.ts"]}"#,
        ),
        ("a.ts", "export const before = 1;"),
        ("first.ts", "import { late } from './z';\nlate;\n"),
    ]);
    let made = std::process::Command::new("mkfifo")
        .arg(fixture.path("z.ts"))
        .status()
        .unwrap();
    assert!(made.success());
    let writer = {
        let path = fixture.path("z.ts");
        std::thread::spawn(move || {
            use std::io::Write;
            let mut pipe = std::fs::OpenOptions::new().write(true).open(path).unwrap();
            std::thread::sleep(std::time::Duration::from_secs(1));
            pipe.write_all(b"export const late = 1;").unwrap();
        })
    };
    let editor = {
        let path = fixture.path("a.ts");
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            std::fs::write(path, "export const after_ = 1;").unwrap();
        })
    };
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    let result = support::references(&mut session, name("before"));
    writer.join().unwrap();
    editor.join().unwrap();
    match result {
        Err(AnalysisError::Input(error)) => {
            assert_eq!(error.kind, InputErrorKind::Io);
            assert!(
                error.message.contains("changed during indexing"),
                "{error:?}"
            );
        }
        other => panic!("a refresh that read a changed input published: {other:?}"),
    }
    assert_eq!(session.generation(), 0);
}

/// A type query after an edit answers from the new generation, although a
/// warm session answers while it validates.
#[test]
fn a_type_query_after_an_edit_answers_from_the_new_generation() {
    let fixture = project(&[("a.ts", "export const value = 1;\n")]);
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    let request = || {
        typescript_parser::project::ProjectRequest::Type(
            typescript_parser::types::TypeRequest::new(typescript_parser::types::TypeQuery::At {
                file: fixture.path("a.ts"),
                position: typescript_parser::source::SourcePosition {
                    line: 1,
                    column: 14,
                },
            }),
        )
    };
    let summary = |session: &mut ProjectSession| match session.execute(request()).unwrap() {
        typescript_parser::project::ProjectResponse::Type(answer) => match *answer {
            typescript_parser::types::TypeAnswer::Selected(result) => result.description.summary,
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    assert_eq!(summary(&mut session), "1");
    assert_eq!(summary(&mut session), "1");
    std::fs::write(fixture.path("a.ts"), "export const value = 'two';\n").unwrap();
    assert_eq!(summary(&mut session), "\"two\"");
    assert_eq!(session.generation(), 2);
}

/// A pipe at `path` whose writer answers each reader with `contents`, after
/// `before(read)` for the reader's one-based read number, until stopped.
#[cfg(unix)]
struct Pipe {
    path: std::path::PathBuf,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    writer: Option<std::thread::JoinHandle<()>>,
}

#[cfg(unix)]
impl Pipe {
    fn new(
        path: std::path::PathBuf,
        contents: &'static [u8],
        before: impl Fn(usize) + Send + 'static,
    ) -> Self {
        let made = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap();
        assert!(made.success());
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let writer = {
            let (path, stop) = (path.clone(), stop.clone());
            std::thread::spawn(move || {
                use std::io::Write;
                for read in 1.. {
                    let mut pipe = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
                    if stop.load(std::sync::atomic::Ordering::SeqCst) {
                        return;
                    }
                    before(read);
                    let _ = pipe.write_all(contents);
                    drop(pipe);
                    // Let the reader see the end before the next writer opens.
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            })
        };
        Self {
            path,
            stop,
            writer: Some(writer),
        }
    }
}

#[cfg(unix)]
impl Drop for Pipe {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        // The writer waits for a reader; this one releases it.
        drop(std::fs::File::open(&self.path));
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

/// An input changed while an earlier input is read from a pipe is seen, as
/// validating in key order sees it: the configuration, a pipe sorting before
/// `z.ts`, changes `z.ts` only once its third reader has begun.
#[cfg(unix)]
#[test]
fn a_change_made_while_an_earlier_pipe_is_read_is_seen() {
    let fixture = project(&[("z.ts", "export const before = 1;\n")]);
    let source = fixture.path("z.ts");
    let config = Pipe::new(
        fixture.path("aconfig.json"),
        br#"{"compilerOptions":{"noLib":true},"files":["z.ts"]}"#,
        move |read| {
            if read == 3 {
                std::fs::write(&source, "export const after_ = 1;\n").unwrap();
            }
        },
    );
    let mut session = ProjectSession::new(&config.path).unwrap();
    // Reads 1 and 2: loading and the check after it.
    assert_eq!(count(&mut session, "before"), 1);
    // Read 3 validates the next request; the edit precedes its end.
    assert_eq!(count(&mut session, "before"), 0);
    assert_eq!(count(&mut session, "after_"), 1);
    assert_eq!(session.generation(), 2);
}

/// A symbolic link retargeted while loading still runs fails the refresh, as
/// its real path changed after loading asked it; the next request publishes
/// the new target. A pipe whose writer answers a second late keeps the first
/// load running.
#[cfg(unix)]
#[test]
fn a_link_retargeted_while_loading_fails_the_refresh_then_publishes() {
    let fixture = project(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"noLib":true},"files":["link.ts","first.ts"]}"#,
        ),
        ("first.ts", "import { late } from './z';\nlate;\n"),
        ("t1.ts", "export const firstTarget = 1;\n"),
        ("t2.ts", "export const secondTarget = 1;\n"),
    ]);
    std::os::unix::fs::symlink(fixture.path("t1.ts"), fixture.path("link.ts")).unwrap();
    let _pipe = Pipe::new(fixture.path("z.ts"), b"export const late = 1;\n", |read| {
        if read == 1 {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
    let retarget = {
        let (link, target) = (fixture.path("link.ts"), fixture.path("t2.ts"));
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            std::fs::remove_file(&link).unwrap();
            std::os::unix::fs::symlink(target, &link).unwrap();
        })
    };
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    let result = support::references(&mut session, name("firstTarget"));
    retarget.join().unwrap();
    match result {
        Err(AnalysisError::Input(error)) => {
            assert!(
                error.message.contains("changed during indexing"),
                "{error:?}"
            )
        }
        other => panic!("a refresh across a retargeted link published: {other:?}"),
    }
    assert_eq!(session.generation(), 0);
    assert_eq!(count(&mut session, "secondTarget"), 1);
    assert_eq!(count(&mut session, "firstTarget"), 0);
    assert_eq!(session.generation(), 1);
}

/// A session loads a source changed while a package manifest is read from a
/// pipe with its new text and publishes it: the manifest's writer, once its
/// first reader opens it, changes `z.ts`, which a worker had already read.
#[cfg(unix)]
#[test]
fn a_source_changed_while_a_manifest_pipe_is_read_is_published_with_its_new_text() {
    let fixture = project(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"module":"nodenext","noLib":true,"types":[]},"files":["sub/a.ts","z.ts"]}"#,
        ),
        ("sub/a.ts", "export const a = 1;\n"),
        ("z.ts", "export const before = 1;\n"),
    ]);
    let source = fixture.path("z.ts");
    let _manifest = Pipe::new(fixture.path("sub/package.json"), b"{}", move |read| {
        if read == 1 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            std::fs::write(&source, "export const after_ = 1;\n").unwrap();
        }
    });
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    assert_eq!(count(&mut session, "after_"), 1);
    assert_eq!(count(&mut session, "before"), 0);
    assert_eq!(session.generation(), 1);
}

/// Package manifests the module-specifier environment found unreadable stay
/// unreadable without making the generation stale, and an edit beside them
/// is still found.
#[cfg(unix)]
#[test]
fn unchanged_unreadable_manifests_keep_the_generation_and_edits_are_found() {
    use std::os::unix::fs::PermissionsExt;
    let mut files = vec![(
        "tsconfig.json".to_owned(),
        r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","noLib":true,"types":[]},"include":["d*/**/*"]}"#.to_owned(),
    )];
    for index in 0..60 {
        files.push((
            format!("d{index:02}/m.ts"),
            format!("export const v{index} = {index};\n"),
        ));
        files.push((format!("d{index:02}/package.json"), "{}".to_owned()));
    }
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let fixture = support::Fixture::new(&borrowed);
    for index in 0..60 {
        let manifest = fixture.path(&format!("d{index:02}/package.json"));
        std::fs::set_permissions(manifest, std::fs::Permissions::from_mode(0o000)).unwrap();
    }
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    assert_eq!(count(&mut session, "v59"), 1);
    assert_eq!(count(&mut session, "v0"), 1);
    assert_eq!(session.generation(), 1);
    std::fs::write(fixture.path("d30/m.ts"), "export const w30 = 30;\n").unwrap();
    assert_eq!(count(&mut session, "w30"), 1);
    assert_eq!(session.generation(), 2);
    for index in 0..60 {
        let manifest = fixture.path(&format!("d{index:02}/package.json"));
        std::fs::set_permissions(manifest, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
}

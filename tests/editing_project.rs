//! Editing in a configured project: selection over the searched files of
//! one generation, which crosses the service boundary as owned data, and
//! binding, which prepares the selected edits against the files on disk.
mod support;

use support::Fixture;
use typescript_parser::analysis::{AnalysisError, QueryErrorKind};
use typescript_parser::editing::{
    self, Edit, EditRequest, Effect, Operation, Outcome, Prepared, RejectionKind, Selected,
};
use typescript_parser::navigation::{ShowOutcome, ShowQuery};
use typescript_parser::project::{ProjectRequest, ProjectResponse, ProjectSession};
use typescript_parser::source::SourcePosition;

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "noLib": true }, "include": ["src"] }"#,
        ),
        (
            "src/host.ts",
            "export function start() { return 1; }\nexport function stop() {}\n",
        ),
        ("src/other.ts", "export function stop() {}\n"),
        ("outside/x.ts", "export function x() {}\n"),
    ])
}

fn select(fixture: &Fixture, edits: Vec<Edit>) -> Result<Selected, AnalysisError> {
    let mut session = ProjectSession::new(&fixture.path("tsconfig.json")).unwrap();
    let mut request = ProjectRequest::Edit(EditRequest { edits });
    request.make_absolute()?;
    session.execute(request).map(|response| match response {
        ProjectResponse::Edit(selected) => *selected,
        other => panic!("an edit request answered {other:?}"),
    })
}

fn replace(name: &str, file: Option<&str>, fixture: &Fixture, body: &str) -> Edit {
    Edit {
        operation: Operation::ReplaceBody,
        target: ShowQuery::Name {
            name: name.into(),
            file: file.map(|file| fixture.path(file)),
        },
        fragment: body.into(),
    }
}

/// Bind a selection that must prepare.
fn bound(selected: Selected) -> typescript_parser::editing::PreparedBatch {
    let Outcome::Ready(selection) = selected else {
        panic!("expected a selection, got {selected:?}");
    };
    match editing::bind(&selection).unwrap() {
        Prepared::Ready(batch) => batch,
        other => panic!("expected a batch, got {other:?}"),
    }
}

#[test]
fn a_name_selects_across_the_searched_files_and_binds_to_disk() {
    let fixture = fixture();
    let Outcome::Ready(selection) = select(
        &fixture,
        vec![replace("start", None, &fixture, "{ return 2; }")],
    )
    .unwrap() else {
        panic!("expected a selection");
    };
    assert_eq!(selection.files.len(), 1);
    let file = &selection.files[0];
    assert_eq!(file.display, "src/host.ts");
    assert_eq!(
        file.text,
        "export function start() { return 1; }\nexport function stop() {}\n"
    );
    // The selection is owned data, as the service sends it.
    let sent: Selected = serde_json::from_str(
        &serde_json::to_string(&Outcome::Ready(selection.clone()) as &Selected).unwrap(),
    )
    .unwrap();
    let batch = bound(sent);
    assert_eq!(batch.files()[0].display(), "src/host.ts");
    assert_eq!(
        batch.files()[0].proposed_text(),
        "export function start() { return 2; }\nexport function stop() {}\n"
    );
    assert_eq!(batch.apply().effect, Effect::Applied);
    assert_eq!(
        std::fs::read_to_string(fixture.path("src/host.ts")).unwrap(),
        "export function start() { return 2; }\nexport function stop() {}\n"
    );
}

#[test]
fn an_ambiguous_name_lists_candidates_in_every_file() {
    let fixture = fixture();
    let Outcome::Unselected(unselected) =
        select(&fixture, vec![replace("stop", None, &fixture, "{}")]).unwrap()
    else {
        panic!("expected an ambiguous selection");
    };
    let ShowOutcome::Ambiguous(candidates) = unselected.outcome else {
        panic!("expected candidates");
    };
    let files: Vec<&str> = candidates
        .iter()
        .map(|candidate| candidate.file.as_str())
        .collect();
    assert_eq!(files, ["src/host.ts", "src/other.ts"]);
    assert_eq!(unselected.file, None);
    // A file narrows the name.
    let batch = bound(
        select(
            &fixture,
            vec![replace("stop", Some("src/other.ts"), &fixture, "{ go(); }")],
        )
        .unwrap(),
    );
    assert_eq!(batch.files()[0].display(), "src/other.ts");
}

#[test]
fn edits_of_several_files_are_selected_in_one_generation() {
    let fixture = fixture();
    let edits = vec![
        replace("start", Some("src/host.ts"), &fixture, "{ return 3; }"),
        Edit {
            operation: Operation::InsertAfter,
            target: ShowQuery::At {
                file: fixture.path("src/other.ts"),
                position: SourcePosition {
                    line: 1,
                    column: 17,
                },
            },
            fragment: "export function resume() {}".into(),
        },
        replace("stop", Some("src/host.ts"), &fixture, "{ halt(); }"),
    ];
    let batch = bound(select(&fixture, edits).unwrap());
    let files: Vec<(&str, &str)> = batch
        .files()
        .iter()
        .map(|file| (file.display(), file.proposed_text()))
        .collect();
    assert_eq!(
        files,
        [
            (
                "src/host.ts",
                "export function start() { return 3; }\nexport function stop() { halt(); }\n"
            ),
            (
                "src/other.ts",
                "export function stop() {}\nexport function resume() {}\n"
            ),
        ]
    );
}

#[test]
fn a_file_outside_the_project_or_a_bad_position_is_a_query_failure() {
    let fixture = fixture();
    let error = select(
        &fixture,
        vec![replace("x", Some("outside/x.ts"), &fixture, "{}")],
    )
    .unwrap_err();
    let AnalysisError::Query(error) = error else {
        panic!("{error:?}");
    };
    assert_eq!(error.kind, QueryErrorKind::OutsideProject);
    let error = select(
        &fixture,
        vec![Edit {
            operation: Operation::ReplaceBody,
            target: ShowQuery::At {
                file: fixture.path("src/host.ts"),
                position: SourcePosition { line: 9, column: 1 },
            },
            fragment: "{}".into(),
        }],
    )
    .unwrap_err();
    let AnalysisError::Query(error) = error else {
        panic!("{error:?}");
    };
    assert_eq!(error.kind, QueryErrorKind::InvalidPosition);
}

#[test]
fn a_file_changed_after_the_project_read_it_does_not_bind() {
    let fixture = fixture();
    let Outcome::Ready(selection) = select(
        &fixture,
        vec![replace("start", None, &fixture, "{ return 2; }")],
    )
    .unwrap() else {
        panic!("expected a selection");
    };
    std::fs::write(
        fixture.path("src/host.ts"),
        "export function start() { return 9; }\nexport function stop() {}\n",
    )
    .unwrap();
    let Outcome::Rejected(rejection) = editing::bind(&selection).unwrap() else {
        panic!("expected a rejection");
    };
    assert_eq!(rejection.kind, RejectionKind::Stale);
    assert_eq!(rejection.edit, Some(0));
}

#[test]
fn a_site_is_selected_in_its_file_when_bound() {
    let fixture = fixture();
    // The project knows `stop` is in src/other.ts; the file's own selection
    // decides the site, and an unsupported one is rejected then.
    let Outcome::Ready(selection) = select(
        &fixture,
        vec![Edit {
            operation: Operation::InsertAfter,
            target: ShowQuery::Name {
                name: "missing".into(),
                file: Some(fixture.path("src/other.ts")),
            },
            fragment: "go();".into(),
        }],
    )
    .unwrap() else {
        panic!("expected a selection");
    };
    let Outcome::Unselected(unselected) = editing::bind(&selection).unwrap() else {
        panic!("expected no selection");
    };
    assert_eq!(unselected.outcome, ShowOutcome::NotFound);
    assert_eq!(unselected.file.as_deref(), Some("src/other.ts"));
}

#[test]
fn a_project_plan_names_its_configuration_whether_or_not_it_loads() {
    let fixture = fixture();
    let batch = bound(
        select(
            &fixture,
            vec![replace("start", None, &fixture, "{ return 2; }")],
        )
        .unwrap(),
    );
    let path = fixture.path("plan.json");
    batch.save(&path).unwrap();
    let names: Vec<_> = typescript_parser::editing::PreparedBatch::load(&path)
        .unwrap()
        .names()
        .into_iter()
        .map(std::path::Path::to_path_buf)
        .collect();
    let canonical = |file: &str| std::fs::canonicalize(fixture.path(file)).unwrap();
    assert_eq!(
        names,
        [
            canonical("tsconfig.json"),
            canonical("src/host.ts"),
            canonical("src/host.ts")
        ]
    );
    let mut plan: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    plan["build"] = "another".into();
    std::fs::write(&path, plan.to_string()).unwrap();
    let error = typescript_parser::editing::PreparedBatch::load(&path).unwrap_err();
    assert_eq!(error.names.unwrap(), names);
}

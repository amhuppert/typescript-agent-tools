//! Documented differences from TypeScript 7.0.2 (`docs/references.md`). Each
//! project in `fixtures/reference-exceptions` records in `expected.json` the
//! pinned compiler's answer and this tool's at each query place. This test
//! checks the tool's side; the real-oracle test in
//! `tools/semantic-oracle.test.mjs` checks the compiler's against the semantic
//! oracle, so a change to either side fails one of them.

mod support;

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use support::Fixture;
use typescript_parser::analysis::{AnalysisError, QueryErrorKind};
use typescript_parser::project::Project;
use typescript_parser::references::{Location, ReferenceQuery};
use typescript_parser::source::SourcePosition;

fn cases() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/reference-exceptions");
    let mut cases: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    cases.sort();
    cases
}

fn place(location: &Location) -> String {
    format!(
        "{}:{}:{}",
        location.file, location.start.line, location.start.column
    )
}

/// The tool's answer at `file:line:column`, in the shape `expected.json`
/// records: the groups, or `unresolved` or `unsupported`.
fn answer(fixture: &Fixture, project: &Project, at: &str) -> Value {
    let mut parts = at.rsplitn(3, ':');
    let column = parts.next().unwrap().parse().unwrap();
    let line = parts.next().unwrap().parse().unwrap();
    let file = parts.next().unwrap();
    let query = ReferenceQuery::At {
        file: fixture.path(file),
        position: SourcePosition { line, column },
    };
    match project.find_references(query) {
        Ok(result) => Value::Array(
            result
                .groups
                .iter()
                .map(|group| {
                    let declarations: Vec<String> = group
                        .declarations
                        .iter()
                        .map(|declaration| place(&declaration.location))
                        .collect();
                    let occurrences: Vec<String> = group
                        .occurrences
                        .iter()
                        .map(|occurrence| place(&occurrence.location))
                        .collect();
                    json!({ "declarations": declarations, "occurrences": occurrences })
                })
                .collect(),
        ),
        Err(AnalysisError::Query(error)) if error.kind == QueryErrorKind::UnresolvedTarget => {
            json!("unresolved")
        }
        Err(AnalysisError::Query(error)) if error.kind == QueryErrorKind::UnsupportedTarget => {
            json!("unsupported")
        }
        Err(error) => panic!("query at {at}: {error}"),
    }
}

#[test]
fn exception_fixtures_answer_as_recorded_and_differ_from_typescript() {
    let cases = cases();
    assert!(!cases.is_empty(), "no exception fixtures");
    for case in cases {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(case.join("expected.json")).unwrap();
        let expected: Value = serde_json::from_str(&text).unwrap();
        let fixture = Fixture::existing(&case);
        let project = fixture.load();
        let queries = expected["queries"].as_array().expect("queries");
        assert!(!queries.is_empty(), "{name}: no queries");
        for query in queries {
            let at = query["at"].as_str().expect("a query place");
            assert_eq!(
                answer(&fixture, &project, at),
                query["tool"],
                "{name}: query at {at}"
            );
        }
        // The compiler side is checked against the oracle; a fixture whose
        // recorded answers all agree no longer shows a difference.
        assert!(
            queries
                .iter()
                .any(|query| query["typescript"] != query["tool"]),
            "{name}: the recorded TypeScript answers equal this tool's"
        );
    }
}

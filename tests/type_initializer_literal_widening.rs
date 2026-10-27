//! `getWidenedLiteralTypeForInitializer` keeps an initializer's fresh literal
//! only for a constant or `isDeclarationReadonly` declaration, which excludes
//! parameter properties. Expected answers are native 7.0.2's
//! (`tools/type-initializer-literal-widening-oracle.test.mjs`).
mod support;
use std::process::{Command, Stdio};
use support::{Fixture, position};
use typescript_parser::references::{
    ReferenceAnswer, ReferenceFilters, ReferenceOutcome, ReferenceQuery, ReferenceRequest,
};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const FIXTURE: &str = "fixtures/types/native-initializer-literal-widening";
const CALL_FIXTURE: &str = "fixtures/types/native-initializer-literal-widening-call";

fn marked(fixture: &Fixture, file: &str, name: &str) -> SourcePosition {
    let source = std::fs::read_to_string(fixture.path(file)).unwrap();
    let marker = format!("/*#{name}*/");
    position(&source, source.find(&marker).unwrap() + marker.len())
}

#[test]
fn initializer_literals_widen_as_native_for_each_source_and_declaration() {
    let fixture = Fixture::copy_from_repository(FIXTURE);
    let expected: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(fixture.path("expected.json")).unwrap())
            .unwrap();
    assert_eq!(expected.len(), 69);
    let project = fixture.load();
    for row in &expected {
        let file = row["file"].as_str().unwrap();
        let name = row["name"].as_str().unwrap();
        assert_eq!(row["errorType"], false, "{name}");
        for expand in [false, true] {
            let answer = project.type_of(TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path(file),
                    position: marked(&fixture, file, name),
                },
                expand,
                depth: 1,
            });
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{file}/{name}/{expand}: {answer:?}")
            };
            assert_eq!(
                result.outcome,
                TypeOutcome::Found,
                "{file}/{name}/{expand}: {:?}",
                result.issues
            );
            assert_eq!(
                result.description.summary,
                row["summary"].as_str().unwrap(),
                "{file}/{name}/{expand}"
            );
            assert!(
                result.issues.is_empty(),
                "{file}/{name}/{expand}: {:?}",
                result.issues
            );
        }
    }
}

#[test]
fn a_member_search_through_a_widened_parameter_property_call_is_complete() {
    let fixture = Fixture::copy_from_repository(FIXTURE);
    let project = fixture.load();
    let answer = project
        .references(ReferenceRequest {
            query: ReferenceQuery::Name {
                name: "Forms.describe".to_owned(),
                file: None,
            },
            filters: ReferenceFilters::default(),
        })
        .unwrap();
    let ReferenceAnswer::Selected(result) = answer else {
        panic!("member selection failed: {answer:?}");
    };
    assert!(result.coverage.gaps.is_empty(), "{result:?}");
    assert_eq!(result.outcome, ReferenceOutcome::Complete, "{result:?}");
    assert_eq!(result.groups.len(), 1, "{result:?}");
    let mut occurrences: Vec<_> = result.groups[0]
        .occurrences
        .iter()
        .map(|occurrence| {
            let location = &occurrence.location;
            (
                location.file.as_str(),
                location.start.line,
                location.start.column,
                location.end.column,
            )
        })
        .collect();
    occurrences.sort();
    assert_eq!(
        occurrences,
        [("calls.ts", 10, 57, 65), ("cases.ts", 79, 3, 11)],
        "{result:?}"
    );
}

/// The smallest shape: an imported constant as a readonly parameter
/// property's default, and a call passing another string constant.
#[test]
fn the_cli_widens_an_imported_constant_parameter_property_and_accepts_the_call() {
    let fixture = Fixture::copy_from_repository(CALL_FIXTURE);
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_tsparse"))
            .args(args)
            .args(["--project", "tsconfig.json", "--direct"])
            .current_dir(&fixture.root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        (output.status.code(), stdout, stderr)
    };
    for (at, summary) in [("a.ts:2:47", "string"), ("b.ts:3:18", "A")] {
        let (code, stdout, stderr) = run(&["--type", "--at", at]);
        assert_eq!(code, Some(0), "{at}: {stdout}{stderr}");
        assert_eq!(stdout.lines().nth(2), Some(summary), "{at}: {stdout}");
    }
    let (code, stdout, stderr) = run(&["--references", "--name", "A.get", "--json"]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let result: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(result["outcome"], "complete", "{stdout}");
    let occurrences: Vec<_> = result["groups"][0]["occurrences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|occurrence| {
            let location = &occurrence["location"];
            format!(
                "{}:{}:{}",
                location["file"].as_str().unwrap(),
                location["start"]["line"],
                location["start"]["column"]
            )
        })
        .collect();
    assert_eq!(occurrences, ["a.ts:2:59", "b.ts:4:24"], "{stdout}");
}

//! Reporting consumes widened display and raw name lookup, not a type read.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-implicit-any/cases.ts");

#[test]
fn implicit_any_reporting_preserves_native_type_results_and_lookup_boundaries() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-implicit-any/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-implicit-any/tsconfig.json"),
        ),
        (
            "suggestions.json",
            include_str!("../fixtures/types/native-implicit-any/suggestions.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-implicit-any/loose.json"),
        ),
    ]);
    for (profile, captured) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-implicit-any/expected-tsconfig.json"),
        ),
        (
            "suggestions",
            include_str!("../fixtures/types/native-implicit-any/expected-suggestions.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-implicit-any/expected-loose.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("{profile}.json"));
        let mut rows: Vec<serde_json::Value> = serde_json::from_str(captured).unwrap();
        assert_eq!(rows.len(), 10);
        for reverse in [false, true] {
            if reverse {
                rows.reverse();
            }
            for row in &rows {
                let name = row["name"].as_str().unwrap();
                assert_eq!(row["errorType"], false);
                let marker = format!("/*#{name}*/");
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len()),
                    });
                    request.expand = expand;
                    request.depth = 2;
                    let answer = project.type_of(request);
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        answer.outcome,
                        TypeOutcome::Found,
                        "{profile}/{name}/{expand}: {answer:?}"
                    );
                    assert_eq!(
                        answer.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}/{expand}"
                    );
                    assert!(
                        answer.issues.is_empty(),
                        "{profile}/{name}/{expand}: {answer:?}"
                    );
                }
            }
        }
    }
}

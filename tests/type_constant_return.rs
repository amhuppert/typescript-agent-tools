//! Native bare-self returns use constant-reference identity, not syntax names.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-constant-return/cases.ts");

#[test]
fn constant_self_returns_preserve_native_identity_and_circular_recovery() {
    for (profile, captured) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-constant-return/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-constant-return/expected-loose.json"),
        ),
    ] {
        let fixture = Fixture::new(&[
            ("cases.ts", SOURCE),
            (
                "globals.d.ts",
                include_str!("../fixtures/types/native-constant-return/globals.d.ts"),
            ),
            (
                "tsconfig.json",
                include_str!("../fixtures/types/native-constant-return/tsconfig.json"),
            ),
            (
                "loose.json",
                include_str!("../fixtures/types/native-constant-return/loose.json"),
            ),
        ]);
        let project = fixture.load_config(&format!("{profile}.json"));
        let rows: Vec<serde_json::Value> = serde_json::from_str(captured).unwrap();
        for row in rows {
            let name = row["name"].as_str().unwrap();
            let circular = row["after"] == serde_json::json!([7023]);
            assert_eq!(row["errorType"], false);
            if !circular {
                assert_eq!(row["after"], serde_json::json!([]));
            }
            let marker = format!("/*#{name}*/");
            let offset = SOURCE.find(&marker).unwrap() + marker.len();
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, offset),
                });
                request.expand = expand;
                request.depth = 3;
                let answer = project.type_of(request);
                if circular {
                    // getReturnTypeOfSignature's rejected resolution frame
                    // reaches 7023 and displays Any. The query retains that
                    // consumed recovery even though its function root is sound.
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert_eq!(
                        error.reason,
                        SemanticFailureReason::CircularType.into(),
                        "{profile}/{name}/{expand}: {error:?}"
                    );
                    assert!(error.omitted_causes.is_none());
                    continue;
                }
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

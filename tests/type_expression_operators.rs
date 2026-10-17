//! Conditional, delete and in operator answers against the pinned native matrix.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-expression-operators/cases.ts");

#[test]
fn conditional_delete_and_in_expressions_preserve_native_dependency_boundaries() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-expression-operators/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-expression-operators/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-expression-operators/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-expression-operators/exact.json"),
        ),
    ]);
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, expected) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-expression-operators/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-expression-operators/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-expression-operators/expected-exact.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("{profile}.json"));
        let expected: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        assert_eq!(expected.len(), 29);
        for row in expected {
            let name = row["name"].as_str().unwrap();
            let marker = format!("/*#{name}*/");
            let selected = SOURCE.find(&marker).unwrap() + marker.len();
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, selected),
                });
                request.expand = expand;
                request.depth = 2;
                let answer = project.type_of(request);
                if name == "conditionalNested" {
                    // Pinned native and API probes establish `true` at this
                    // inner read, differing from its declared boolean. The
                    // selected T profile therefore requires flow admission.
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(error.kind, SemanticErrorKind::UnsupportedSemantic);
                    assert_eq!(error.reason, SemanticFailureReason::FlowRequired.into());
                    assert_eq!(error.path.as_deref(), Some(path.as_path()));
                    assert_eq!(
                        error.position,
                        Some(position(
                            SOURCE,
                            SOURCE.find("booleanValue ? 1 : 2").unwrap()
                        ))
                    );
                    continue;
                }
                let missing = !row["symbolError"].is_null()
                    || matches!(name, "deleteHidden" | "conditionalBothArms");
                let unknown =
                    profile != "loose" && matches!(name, "inUnknownReceiver" | "inUnknownKey");
                if missing || unknown || row["errorType"] == true {
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(error.path.as_deref(), Some(path.as_path()));
                    let (reason, offset) = if missing {
                        (
                            SemanticFailureReason::UnresolvedSymbol,
                            SOURCE.find("Missing").unwrap(),
                        )
                    } else {
                        {
                            let initializer =
                                selected + SOURCE[selected..].find(" = ").unwrap() + 3;
                            (
                                SemanticFailureReason::ErrorType,
                                initializer + SOURCE[initializer..].find("unknownValue").unwrap(),
                            )
                        }
                    };
                    assert_eq!(error.reason, reason.into(), "{profile}/{name}: {error:?}");
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, offset)),
                        "{profile}/{name}: {error:?}"
                    );
                } else {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        result.outcome,
                        TypeOutcome::Found,
                        "{profile}/{name}: {result:?}"
                    );
                    assert_eq!(
                        result.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}/{expand}"
                    );
                    assert!(result.issues.is_empty(), "{profile}/{name}: {result:?}");
                }
            }
        }
    }
}

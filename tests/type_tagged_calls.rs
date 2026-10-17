//! Tagged templates use their actual synthetic and substitution arguments.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-tagged-calls/cases.ts");

#[test]
fn tagged_calls_share_native_selection_with_template_argument_and_failure_provenance() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-tagged-calls/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-tagged-calls/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-tagged-calls/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-tagged-calls/exact.json"),
        ),
    ]);
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, expected) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-tagged-calls/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-tagged-calls/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-tagged-calls/expected-exact.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("{profile}.json"));
        let expected: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        assert_eq!(expected.len(), 16);
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
                let missing = name == "untypedMissing";
                if missing || matches!(name, "uncallable" | "arrayUncallable") {
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
                            SOURCE.find("MissingSubstitution").unwrap(),
                        )
                    } else {
                        let initializer = selected + SOURCE[selected..].find(" = ").unwrap() + 3;
                        (
                            SemanticFailureReason::ErrorType,
                            initializer + usize::from(name == "arrayUncallable"),
                        )
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

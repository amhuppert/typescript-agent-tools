//! Complete assignment elaboration is observable through final type answers
//! and diagnostic-only dependencies, not through diagnostic text snapshots.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-elaboration/cases.ts");

#[test]
fn assignments_and_satisfies_share_native_elaboration_and_failure_provenance() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-elaboration/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-elaboration/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-elaboration/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-elaboration/exact.json"),
        ),
    ]);
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, expected) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-elaboration/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-elaboration/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-elaboration/expected-exact.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("{profile}.json"));
        let expected: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        assert_eq!(expected.len(), 38);
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
                // These local receivers have no initializer. The compiler's
                // definite-assignment flow changes their initial read in strict
                // mode, which remains outside the admitted flow profile.
                if profile != "loose"
                    && matches!(
                        name,
                        "assignOptional"
                            | "compoundGetterSetter"
                            | "compoundParenthesizedGetterSetter"
                            | "compoundReadonly"
                    )
                {
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(error.kind, SemanticErrorKind::UnsupportedSemantic);
                    assert_eq!(error.reason, SemanticFailureReason::FlowRequired.into());
                    continue;
                }
                let missing = !row["symbolError"].is_null();
                if missing || row["errorType"] == true {
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
                        (
                            SemanticFailureReason::ErrorType,
                            SOURCE.rfind("readonlyTarget.a").unwrap() + "readonlyTarget.".len(),
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

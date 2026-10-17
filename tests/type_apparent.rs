//! Apparent conversion is observed through an actual property-assignment read.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-apparent/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-apparent/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-apparent/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-apparent/loose.json"),
        ),
        (
            "unchecked.json",
            include_str!("../fixtures/types/native-apparent/unchecked.json"),
        ),
        (
            "missing.json",
            include_str!("../fixtures/types/native-apparent/missing.json"),
        ),
        (
            "arity.json",
            include_str!("../fixtures/types/native-apparent/arity.json"),
        ),
        (
            "kind.json",
            include_str!("../fixtures/types/native-apparent/kind.json"),
        ),
        (
            "missing.d.ts",
            include_str!("../fixtures/types/native-apparent/missing.d.ts"),
        ),
        (
            "arity.d.ts",
            include_str!("../fixtures/types/native-apparent/arity.d.ts"),
        ),
        (
            "kind.d.ts",
            include_str!("../fixtures/types/native-apparent/kind.d.ts"),
        ),
    ])
}

fn markers() -> Vec<String> {
    serde_json::from_str(include_str!(
        "../fixtures/types/native-apparent/markers.json"
    ))
    .unwrap()
}

#[test]
fn primitive_wrappers_and_constraints_use_their_native_members_lazily() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "unchecked.json"] {
        let project = fixture.load_config(config);
        for marker in markers() {
            let tag = format!("/*#{marker}*/");
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
                });
                request.expand = expand;
                request.depth = 2;
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}: actual apparent members required: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{marker}");
                assert_eq!(
                    result.description.summary, "string | number",
                    "{config}/{marker}"
                );
                assert!(
                    result.issues.is_empty(),
                    "{config}/{marker}: unrelated latent read"
                );
            }
        }
    }
}

#[test]
fn consumed_global_wrapper_recovery_is_not_a_healthy_declared_union() {
    let fixture = fixture();
    for config in ["missing.json", "arity.json", "kind.json"] {
        let project = fixture.load_config(config);
        for marker in markers()
            .into_iter()
            .filter(|marker| marker != "constrained")
        {
            let tag = format!("/*#{marker}*/");
            let answer = project.type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
            }));
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{config}/{marker}: global recovery must remain invalid: {answer:?}");
            };
            assert_eq!(
                error.kind,
                SemanticErrorKind::InvalidSemantic,
                "{config}/{marker}"
            );
            assert_eq!(
                error.reason,
                SemanticFailureReason::ErrorType.into(),
                "{config}/{marker}"
            );
            // Property lookup and applicable-index lookup each consume the
            // apparent wrapper, followed by the destructuring errorType.
            assert_eq!(error.causes.len(), 4, "{config}/{marker}: {error:?}");
            assert_eq!(error.causes[1].reason, error.reason);
            assert_eq!(error.causes[0], error.causes[2]);
            assert_eq!(
                error.causes[3].reason,
                SemanticFailureReason::ErrorType.into()
            );
            assert!(error.causes[3].message.contains("destructured"));
            let cause = &error.causes[0];
            if config == "missing.json" {
                assert_eq!(cause.reason, SemanticFailureReason::UnresolvedSymbol.into());
                assert!(cause.location.is_none());
            } else {
                assert_eq!(cause.reason, SemanticFailureReason::ErrorType.into());
                assert_eq!(
                    cause.location.as_ref().unwrap().file,
                    if config == "arity.json" {
                        "arity.d.ts"
                    } else {
                        "kind.d.ts"
                    }
                );
            }
        }
    }
}

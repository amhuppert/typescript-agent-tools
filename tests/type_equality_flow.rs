//! Native equality flow identities remain distinct from excluded refinements.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-equality-flow/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-equality-flow/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-equality-flow/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-equality-flow/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-equality-flow/exact.json"),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
    });
    request.expand = expand;
    request
}

#[test]
fn unchanged_native_equality_flow_remains_healthy() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for marker in [
            "unrelatedStrict",
            "unrelatedLoose",
            "unrelatedNotStrict",
            "unrelatedNotLoose",
            "selfStrict",
            "selfLoose",
            "selfNotStrict",
            "selfNotLoose",
            "booleanRight",
            "booleanLeft",
            "typeofSame",
            "typeofSwapped",
            "typeofOther",
            "anySkips",
            "anySwappedSkips",
            "optionalSame",
            "constructorInequality",
            "constructorNumber",
        ] {
            let expected = match marker {
                "anySkips" | "anySwappedSkips" => "any",
                "optionalSame" if config == "loose.json" => "{ value: number; }",
                "optionalSame" => "{ value: number; } | undefined",
                "constructorInequality" | "constructorNumber" => "HasConstructor",
                _ => "number",
            };
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{marker}/{expand}: {result:?}"
                );
                assert_eq!(
                    result.description.summary, expected,
                    "{config}/{marker}/{expand}: {result:?}"
                );
                assert!(
                    result.issues.is_empty(),
                    "{config}/{marker}/{expand}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn actual_native_refinements_keep_the_existing_flow_boundary() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for marker in [
            "literalChanges",
            "swappedChanges",
            "negativeChanges",
            "nullableChanges",
            "nullableLoose",
            "optionalChanges",
            "typeofChanges",
            "discriminantChanges",
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                if config == "loose.json"
                    && matches!(
                        marker,
                        "nullableChanges" | "nullableLoose" | "optionalChanges"
                    )
                {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{config}/{marker}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        result.outcome,
                        TypeOutcome::Found,
                        "{config}/{marker}/{expand}: {result:?}"
                    );
                    assert_eq!(
                        result.description.summary,
                        if marker == "optionalChanges" {
                            "{ value: number; }"
                        } else {
                            "number"
                        }
                    );
                } else {
                    assert!(
                        matches!(&answer, Err(AnalysisError::Semantic(error))
                        if error.kind == SemanticErrorKind::UnsupportedSemantic
                        && error.reason == SemanticFailureReason::FlowRequired.into()),
                        "{config}/{marker}/{expand}: {answer:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn acquired_opposite_error_stays_invalid_after_native_identity_recovery() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, "matchedMissing", expand));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::InvalidSemantic
                && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
                "{config}/{expand}: {answer:?}"
            );
        }
    }
}

//! Synthetic tuple slots use the same expression questions as captured arguments.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-synthetic-expressions/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-synthetic-expressions/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-synthetic-expressions/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-synthetic-expressions/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-synthetic-expressions/exact.json"),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth: 0,
    }
}
#[test]
fn positional_spreads_are_indexed_while_rest_construction_retains_raw_types() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [
            ("ordinary", "string"),
            ("spread", "number"),
            ("rawRest", "[flag: boolean, ...values: number[]]"),
            ("generic", "<T>(value: T) => T"),
            ("healthyAfter", "number"),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{marker}: {:?}",
                    result.issues
                );
                assert_eq!(result.description.summary, summary, "{config}/{marker}");
                assert!(result.issues.is_empty());
            }
        }
    }
}
#[test]
fn synthetic_generic_signature_postcheck_preserves_its_consumed_dependency() {
    let fixture = fixture();
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for marker in ["consumed", "healthyAfter", "consumed"] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                if marker == "healthyAfter" {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{config}/{marker}: {answer:?}");
                    };
                    assert_eq!(result.outcome, TypeOutcome::Found);
                    assert_eq!(result.description.summary, "number");
                    assert!(result.issues.is_empty());
                } else {
                    assert!(
                        matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into() && error.path.as_deref() == Some(path.as_path()) && error.position == Some(position(SOURCE,SOURCE.find("MissingSyntheticIndex").unwrap()))),
                        "{config}/{marker}: {answer:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn contextual_rest_indexes_share_concrete_optional_and_index_signature_lookup() {
    let fixture = fixture();
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for marker in [
            "fixedContext",
            "optionalContext",
            "arrayContext",
            "brokenContext",
            "fixedContext",
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                if marker == "brokenContext" {
                    assert!(
                        matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into() && error.path.as_deref() == Some(path.as_path()) && error.position == Some(position(SOURCE,SOURCE.find("MissingContextualIndex").unwrap()))),
                        "{config}/{marker}: {answer:?}"
                    );
                } else {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{config}/{marker}: {answer:?}");
                    };
                    assert_eq!(result.outcome, TypeOutcome::Found);
                    assert_eq!(result.description.summary, "number", "{config}/{marker}");
                    assert!(result.issues.is_empty());
                }
            }
        }
    }
}

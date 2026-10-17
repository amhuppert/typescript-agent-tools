//! Direct property values preserve exact names and their actual checking prelude.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-pending-values/values.ts");
const CONFIG: &str = include_str!("../fixtures/types/native-pending-values/tsconfig.json");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("tsconfig.json", CONFIG),
        ("values.ts", SOURCE),
        (
            "object.json",
            include_str!("../fixtures/types/native-pending-values/object.json"),
        ),
        (
            "object.d.ts",
            include_str!("../fixtures/types/native-pending-values/object.d.ts"),
        ),
    ])
}
fn offset(marker: &str) -> usize {
    let marker = format!("/*#{marker}*/");
    SOURCE.find(&marker).unwrap() + marker.len()
}
fn query(fixture: &Fixture, offset: usize) -> TypeRequest {
    TypeRequest::new(TypeQuery::At {
        file: fixture.path("values.ts"),
        position: position(SOURCE, offset),
    })
}

#[test]
fn outside_call_unpaired_own_property_names_fail_without_replacement() {
    let fixture = fixture();
    let project = fixture.load_config("tsconfig.json");
    for marker in ["outsideHigh", "outsideLow"] {
        for inside in [0, 1, 7] {
            let answer = project.type_of(query(&fixture, offset(marker) + inside));
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("an exact Rust identity cannot replace an unpaired UTF-16 name: {marker}")
            };
            assert_eq!(error.kind, SemanticErrorKind::UnsupportedSemantic);
            assert_eq!(
                error.reason,
                SemanticFailureReason::UnrepresentableIdentity.into()
            );
            assert_eq!(error.position, Some(position(SOURCE, offset(marker))));
            assert!(error.message.contains("unpaired"));
        }
    }
}

#[test]
fn inside_call_unpaired_names_fail_in_the_actual_property_name_prelude() {
    let fixture = fixture();
    let project = fixture.load_config("tsconfig.json");
    for marker in ["insideHigh", "insideLow"] {
        let start = offset(marker);
        for inside in [0, 1, 7] {
            let answer = project.type_of(query(&fixture, start + inside));
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("an exact property name cannot replace an unpaired surrogate: {marker}")
            };
            assert_eq!(error.kind, SemanticErrorKind::UnsupportedSemantic);
            assert_eq!(
                error.reason,
                SemanticFailureReason::UnrepresentableIdentity.into()
            );
            assert_eq!(error.position, Some(position(SOURCE, start)), "{marker}");
            assert!(
                error.message.contains("effective property name"),
                "the call prelude acquires the grammar name before the selected value: {error:?}"
            );
        }
    }
}

#[test]
fn scalar_own_names_return_the_native_value_after_the_call_prelude() {
    let fixture = fixture();
    let project = fixture.load_config("object.json");
    // The compiler hook proves that the call prelude reaches Object's member
    // table. Only this twin has a genuine Object instead of recovery.
    for marker in [
        "insidePair",
        "insideCooked",
        "insideNumeric",
        "insideIdentifier",
    ] {
        for inside in [0, 1] {
            for expand in [false, true] {
                let mut request = query(&fixture, offset(marker) + inside);
                request.expand = expand;
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{marker}/{inside}/{expand}: {answer:?}")
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{marker}/{inside}/{expand}"
                );
                assert_eq!(
                    result.description.summary, "number",
                    "{marker}/{inside}/{expand}"
                );
                assert!(
                    result.issues.is_empty(),
                    "{marker}/{inside}/{expand}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn scalar_own_names_consume_missing_object_only_in_the_call_prelude() {
    let fixture = fixture();
    let project = fixture.load_config("tsconfig.json");
    let path = fixture.path("values.ts").canonicalize().unwrap();
    for marker in [
        "insidePair",
        "insideCooked",
        "insideNumeric",
        "insideIdentifier",
    ] {
        let start = position(SOURCE, offset(marker));
        for inside in [0, 1] {
            for expand in [false, true] {
                let mut request = query(&fixture, offset(marker) + inside);
                request.expand = expand;
                let answer = project.type_of(request);
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!("{marker}/{inside}/{expand}: {answer:?}")
                };
                assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
                assert_eq!(error.reason, SemanticFailureReason::ErrorType.into());
                assert_eq!(error.path.as_deref(), Some(path.as_path()));
                assert_eq!(error.position, Some(start));
                assert!(error.message.contains("global type 'Object'"), "{error:?}");
                assert!(
                    error.causes.iter().any(|cause| cause.reason
                        == SemanticFailureReason::UnresolvedSymbol.into()
                        && cause.location.is_none()
                        && cause.message == "Cannot find global type 'Object'."),
                    "{error:?}"
                );
                for cause in &error.causes {
                    assert!(cause.message.contains("global type 'Object'"), "{error:?}");
                    match &cause.location {
                        None => {
                            assert_eq!(cause.reason, SemanticFailureReason::UnresolvedSymbol.into())
                        }
                        Some(location) => {
                            assert_eq!(cause.reason, SemanticFailureReason::ErrorType.into());
                            assert_eq!(location.file, "values.ts");
                            assert_eq!(location.start, start);
                        }
                    }
                }
                assert!(error.omitted_causes.is_none(), "{error:?}");
            }
        }
    }
}

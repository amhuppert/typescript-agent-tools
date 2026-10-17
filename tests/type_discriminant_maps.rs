//! Compiler-derived union key-map boundaries, observed through real calls.
//! Payload failures distinguish map selection from ordinary candidate replay.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-discriminant-maps/cases.ts");

fn check(cases: &[(&str, Option<&str>)]) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-discriminant-maps/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-discriminant-maps/tsconfig.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-discriminant-maps/exact.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-discriminant-maps/loose.json"),
        ),
    ]);
    for config in ["tsconfig.json", "exact.json", "loose.json"] {
        let project = fixture.load_config(config);
        for &(marker, missing) in cases {
            for expand in [false, true] {
                let marker_text = format!("/*#{marker}*/");
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(
                        SOURCE,
                        SOURCE.find(&marker_text).unwrap() + marker_text.len(),
                    ),
                });
                request.expand = expand;
                let answer = project.type_of(request);
                if let Some(missing) = missing {
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{config}/{marker}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{config}/{marker}"
                    );
                    assert_eq!(
                        error.reason,
                        SemanticFailureReason::UnresolvedSymbol.into(),
                        "{config}/{marker}"
                    );
                    assert_eq!(
                        error.path.as_deref(),
                        Some(fixture.path("cases.ts").as_path()),
                        "{config}/{marker}"
                    );
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, SOURCE.find(missing).unwrap())),
                        "{config}/{marker}"
                    );
                } else {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{config}/{marker}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        result.outcome,
                        TypeOutcome::Found,
                        "{config}/{marker}: {:?}",
                        result.issues
                    );
                    assert_eq!(
                        result.description.summary, "\"matched\"",
                        "{config}/{marker}/{expand}"
                    );
                    assert!(
                        result.issues.is_empty(),
                        "{config}/{marker}: {:?}",
                        result.issues
                    );
                }
            }
        }
    }
}

#[test]
fn map_size_gates_precede_discriminant_dependencies() {
    check(&[
        ("underNine", None),
        ("atTen", Some("MissingDiscriminant")),
        ("overEleven", Some("MissingDiscriminant")),
        ("objectCountNine", None),
        ("healthyNine", None),
        ("healthyTen", None),
        ("healthyEleven", None),
        ("nonLiteralTen", None),
        ("emptyName", None),
    ]);
}

#[test]
fn half_threshold_and_duplicate_map_counts_preserve_native_answers() {
    check(&[
        ("aboveHalf", None),
        ("half", None),
        ("belowHalf", None),
        ("duplicateTen", None),
        ("duplicateEleven", None),
        ("duplicateThird", None),
    ]);
}

#[test]
fn selected_constituents_skip_payloads_but_rejected_maps_consume_them() {
    check(&[
        ("payloadUnderNine", Some("MissingPayload")),
        ("payloadAtTen", None),
        ("payloadOverEleven", None),
        ("payloadHalf", None),
        ("payloadBelowHalf", Some("MissingPayload")),
    ]);
}

#[test]
fn ambiguous_key_replays_candidates_and_third_duplicate_keeps_native_count() {
    check(&[
        ("payloadDuplicateTen", Some("MissingPayload")),
        ("payloadDuplicateEleven", None),
        ("payloadAmbiguous", Some("MissingPayload")),
        ("payloadThirdDuplicate", None),
    ]);
}

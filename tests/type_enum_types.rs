//! Pinned getDeclaredTypeOfEnum/getEnumMemberValue and consumer distinctions.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-enum-types/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "other.ts",
            include_str!("../fixtures/types/native-enum-types/other.ts"),
        ),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-enum-types/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-enum-types/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-enum-types/loose.json"),
        ),
        (
            "isolated.json",
            include_str!("../fixtures/types/native-enum-types/isolated.json"),
        ),
    ])
}

fn query(fixture: &Fixture, name: &str, expand: bool) -> TypeRequest {
    let marker = format!("/*#{name}*/");
    let mut matches = SOURCE.match_indices(&marker);
    let (offset, _) = matches.next().unwrap();
    assert!(matches.next().is_none(), "ambiguous {marker}");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, offset + marker.len()),
        },
        expand,
        depth: usize::from(expand),
    }
}

#[test]
fn enum_identities_names_constants_and_widening_match_pinned_types() {
    let fixture = fixture();
    for (profile, expected) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-enum-types/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-enum-types/expected-loose.json"),
        ),
        (
            "isolated",
            include_str!("../fixtures/types/native-enum-types/expected-isolated.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("{profile}.json"));
        let rows: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        assert_eq!(rows.len(), 38);
        for row in rows {
            let name = row["name"].as_str().unwrap();
            if matches!(name, "computedBadType" | "badThenLater" | "literalMismatch") {
                continue;
            }
            assert_eq!(row["errorType"], false, "{profile}/{name}");
            for expand in [false, true] {
                let answer = project.type_of(query(&fixture, name, expand));
                let Ok(TypeAnswer::Selected(answer)) = answer else {
                    panic!("{profile}/{name}/{expand}: {answer:?}");
                };
                assert_eq!(
                    answer.outcome,
                    TypeOutcome::Found,
                    "{profile}/{name}: {answer:?}"
                );
                assert_eq!(
                    answer.description.summary,
                    row["summary"].as_str().unwrap(),
                    "{profile}/{name}/{expand}"
                );
                assert!(!answer.description.summary_truncated, "{profile}/{name}");
                assert!(answer.issues.is_empty(), "{profile}/{name}: {answer:?}");
            }
        }
    }
}

#[test]
fn enum_computation_retains_bad_initializer_and_continues_to_later_member() {
    let fixture = fixture();
    for profile in ["tsconfig", "loose", "isolated"] {
        let project = fixture.load_config(&format!("{profile}.json"));
        for name in ["computedBadType", "badThenLater"] {
            for expand in [false, true] {
                let answer = project.type_of(query(&fixture, name, expand));
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!("{profile}/{name}/{expand}: {answer:?}");
                };
                assert_eq!(
                    error.kind,
                    SemanticErrorKind::InvalidSemantic,
                    "{profile}/{name}: {error:?}"
                );
                assert_eq!(
                    error.reason,
                    SemanticFailureReason::UnresolvedSymbol.into(),
                    "{profile}/{name}: {error:?}"
                );
                assert_eq!(
                    error.position,
                    Some(position(SOURCE, SOURCE.find("MissingEnumReturn").unwrap())),
                    "{profile}/{name}: {error:?}"
                );
                assert!(error.causes.iter().all(|cause| cause.reason == SemanticFailureReason::UnresolvedSymbol.into()), "{profile}/{name}: {error:?}");
                if name == "badThenLater" {
                    let later = position(SOURCE, SOURCE.find("MissingLaterEnumReturn").unwrap());
                    assert!(
                        error
                            .causes
                            .iter()
                            .any(|cause| cause
                                .location
                                .as_ref()
                                .is_some_and(|location| location.file == "cases.ts"
                                    && location.start == later)),
                        "{profile}/{name}: {error:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn numeric_enum_literal_mismatch_retains_actual_overload_recovery() {
    let fixture = fixture();
    for profile in ["tsconfig", "loose", "isolated"] {
        let project = fixture.load_config(&format!("{profile}.json"));
        for expand in [false, true] {
            let answer = project.type_of(query(&fixture, "literalMismatch", expand));
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{profile}/{expand}: {answer:?}");
            };
            assert_eq!(
                error.reason,
                SemanticFailureReason::OverloadRecovery.into(),
                "{profile}: {error:?}"
            );
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
            assert!(
                error
                    .causes
                    .iter()
                    .all(|cause| cause.reason == SemanticFailureReason::OverloadRecovery.into()),
                "{profile}: {error:?}"
            );
            assert!(error.omitted_causes.is_none());
        }
    }
}

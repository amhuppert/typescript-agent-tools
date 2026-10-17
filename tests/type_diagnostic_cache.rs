//! Natural repeated calls share one evaluator's diagnostic-node cache.
//! The native observer proves cache transitions; these final queries reject
//! any lost recovery or unfinished display dependency.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-diagnostic-cache/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("native-diagnostic-cache/cases.ts", SOURCE),
        (
            "native-variable-patterns/globals.d.ts",
            include_str!("../fixtures/types/native-variable-patterns/globals.d.ts"),
        ),
        (
            "native-diagnostic-cache/tsconfig.json",
            include_str!("../fixtures/types/native-diagnostic-cache/tsconfig.json"),
        ),
        (
            "native-diagnostic-cache/loose.json",
            include_str!("../fixtures/types/native-diagnostic-cache/loose.json"),
        ),
        (
            "native-diagnostic-cache/untruncated.json",
            include_str!("../fixtures/types/native-diagnostic-cache/untruncated.json"),
        ),
    ])
}

fn query(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("native-diagnostic-cache/cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth: 1,
    }
}

#[test]
fn cold_warm_and_distinct_display_scopes_retain_only_actual_overload_recovery() {
    let fixture = fixture();
    for profile in ["tsconfig", "loose", "untruncated"] {
        let project = fixture.load_config(&format!("native-diagnostic-cache/{profile}.json"));
        for marker in [
            "cold",
            "warm",
            "different",
            "longCold",
            "longWarm",
            "thisCold",
            "thisWarm",
            "recursive",
        ] {
            for expand in [false, true] {
                let answer = project.type_of(query(&fixture, marker, expand));
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!("{profile}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(
                    error.kind,
                    SemanticErrorKind::InvalidSemantic,
                    "{profile}/{marker}: {error:?}"
                );
                assert_eq!(
                    error.reason,
                    SemanticFailureReason::OverloadRecovery.into(),
                    "{profile}/{marker}: {error:?}"
                );
                assert!(error.causes.iter().all(|cause| cause.reason == SemanticFailureReason::OverloadRecovery.into()), "{profile}/{marker}: {error:?}");
                assert!(
                    error.omitted_causes.is_none(),
                    "{profile}/{marker}: {error:?}"
                );
            }
        }
    }
}

#[test]
fn cached_display_replays_the_actual_missing_annotation() {
    let fixture = fixture();
    let missing_start = SOURCE.find("MissingCache").unwrap();
    let expected = position(SOURCE, missing_start);
    let expected_end = position(SOURCE, missing_start + "MissingCache".len());
    let warm_start = SOURCE.find("take(recovered), take(recovered)").unwrap();
    let second_start = warm_start + "take(recovered), ".len();
    for profile in ["tsconfig", "loose", "untruncated"] {
        let project = fixture.load_config(&format!("native-diagnostic-cache/{profile}.json"));
        for marker in ["recoveredCold", "recoveredWarm"] {
            let warm = marker == "recoveredWarm";
            let call_start = if warm {
                warm_start
            } else {
                SOURCE.find("take(recovered /*#recoveredCold*/").unwrap()
            };
            for expand in [false, true] {
                let answer = project.type_of(query(&fixture, marker, expand));
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!("{profile}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(
                    error.kind,
                    SemanticErrorKind::InvalidSemantic,
                    "{profile}/{marker}: {error:?}"
                );
                assert_eq!(
                    error.reason,
                    SemanticFailureReason::OverloadRecovery.into(),
                    "{profile}/{marker}: {error:?}"
                );
                assert_eq!(
                    error.position,
                    Some(position(SOURCE, call_start)),
                    "{profile}/{marker}: {error:?}"
                );
                // Each display retains five actual acquisitions. The second
                // call also reads the first Failed relation twice, once before
                // and once after its overload marker. See the native trace and
                // slot accounting in docs/types-diagnostic-cache-evidence.md.
                let retained_missing = if warm { 15 } else { 10 };
                assert_eq!(
                    error.causes.len(),
                    retained_missing + usize::from(warm),
                    "{profile}/{marker}: {error:?}"
                );
                for (index, cause) in error.causes.iter().enumerate() {
                    let location = cause.location.as_ref().expect("actual dependency range");
                    assert_eq!(location.file, "cases.ts");
                    if index < retained_missing {
                        assert_eq!(
                            cause.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{profile}/{marker}: {error:?}"
                        );
                        assert_eq!(location.start, expected, "{profile}/{marker}: {error:?}");
                        assert_eq!(location.end, expected_end, "{profile}/{marker}: {error:?}");
                    } else {
                        assert_eq!(
                            cause.reason,
                            SemanticFailureReason::OverloadRecovery.into(),
                            "{profile}/{marker}: {error:?}"
                        );
                        assert_eq!(
                            location.start,
                            position(SOURCE, second_start),
                            "{profile}/{marker}: {error:?}"
                        );
                        assert_eq!(
                            location.end,
                            position(SOURCE, second_start + "take(recovered)".len()),
                            "{profile}/{marker}: {error:?}"
                        );
                    }
                }
                assert_eq!(
                    error.omitted_causes,
                    warm.then_some(15),
                    "{profile}/{marker}: {error:?}"
                );
            }
        }
    }
}

#[test]
fn equal_byte_qualification_and_compatible_control_preserve_checked_result() {
    let fixture = fixture();
    for profile in ["tsconfig", "loose", "untruncated"] {
        let project = fixture.load_config(&format!("native-diagnostic-cache/{profile}.json"));
        for marker in ["sameBytes", "equalControl"] {
            for expand in [false, true] {
                let answer = project.type_of(query(&fixture, marker, expand));
                let Ok(TypeAnswer::Selected(answer)) = answer else {
                    panic!("{profile}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(answer.outcome, TypeOutcome::Found);
                assert_eq!(answer.description.summary, "\"checked\"");
                assert!(answer.issues.is_empty());
            }
        }
    }
}

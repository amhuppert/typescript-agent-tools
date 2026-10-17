//! Native overload recovery constructs its signature before reporting failure.
//! A native summary (including genuine Any or Never) does not erase that result.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-overload-failure-signatures/cases.ts");

fn offset(text: &str) -> usize {
    let mut found = SOURCE.match_indices(text);
    let first = found
        .next()
        .unwrap_or_else(|| panic!("missing selector {text}"));
    assert!(found.next().is_none(), "ambiguous selector {text}");
    first.0
}

#[test]
fn overload_factory_keeps_all_native_children_and_distinguishes_quick_controls() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-overload-failure-signatures/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-overload-failure-signatures/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-overload-failure-signatures/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-overload-failure-signatures/exact.json"),
        ),
    ]);
    for (profile, expected) in [
        (
            "tsconfig",
            include_str!(
                "../fixtures/types/native-overload-failure-signatures/expected-tsconfig.json"
            ),
        ),
        (
            "loose",
            include_str!(
                "../fixtures/types/native-overload-failure-signatures/expected-loose.json"
            ),
        ),
        (
            "exact",
            include_str!(
                "../fixtures/types/native-overload-failure-signatures/expected-exact.json"
            ),
        ),
    ] {
        let native: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        assert_eq!(native.len(), 20);
        let project = fixture.load_config(&format!("{profile}.json"));
        for reverse in [false, true] {
            let mut cases: Vec<_> = native.iter().collect();
            if reverse {
                cases.reverse();
            }
            for row in cases {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let selected = offset(&marker) + marker.len();
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, selected),
                    });
                    request.expand = expand;
                    let answer = project.type_of(request);
                    if matches!(
                        name,
                        "selectedSkipsReturn" | "singleCandidateControl" | "healthyIndependent"
                    ) {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{profile}/{name}/{expand}: {answer:?}")
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found);
                        assert_eq!(answer.description.summary, row["summary"].as_str().unwrap());
                        assert_eq!(row["errorType"], false);
                        assert!(answer.issues.is_empty());
                        continue;
                    }
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}")
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}: {error:?}"
                    );
                    let mut events = vec![(error.reason, error.position)];
                    events.extend(error.causes.iter().map(|cause| {
                        let location = cause
                            .location
                            .as_ref()
                            .expect("these native children have actual source sites");
                        assert_eq!(location.file, "cases.ts");
                        (cause.reason, Some(location.start))
                    }));
                    assert!(
                        events
                            .iter()
                            .any(|(reason, _)| *reason
                                == SemanticFailureReason::OverloadRecovery.into()),
                        "native fallback signature must remain recovery: {profile}/{name}: {error:?}"
                    );
                    assert!(
                        events.iter().all(|(reason, _)| *reason
                            == SemanticFailureReason::OverloadRecovery.into()
                            || *reason == SemanticFailureReason::UnresolvedSymbol.into()),
                        "no unrelated pending/error substitute: {profile}/{name}: {error:?}"
                    );
                    let missing: &[&str] = match name {
                        "allMissingReturns" => &["MissingFirstReturn", "MissingSecondReturn"],
                        "positionsBeforeReturns" => &[
                            "MissingFirstParameter",
                            "MissingSecondParameter",
                            "MissingPositionReturnA",
                            "MissingPositionReturnB",
                        ],
                        "unevenRawCounts" => &["MissingExtraPosition"],
                        "restBeforeReturns" => &[
                            "MissingRestElement",
                            "MissingRestReturnA",
                            "MissingRestReturnB",
                        ],
                        "thisBeforeReturns" => &[
                            "MissingFirstThis",
                            "MissingSecondThis",
                            "MissingThisReturnA",
                            "MissingThisReturnB",
                        ],
                        _ => &[],
                    };
                    for missing in missing {
                        assert!(
                            events.iter().any(|(reason, at)| *reason
                                == SemanticFailureReason::UnresolvedSymbol.into()
                                && *at == Some(position(SOURCE, offset(missing)))),
                            "missing native child {missing}: {profile}/{name}: {error:?}"
                        );
                    }
                    // These calls reject at their first argument, so applicability
                    // never reads their later optional slots or return types.
                    // The factory acquires each exactly once, then publishes its
                    // recovery signature before the reporting tail.
                    if matches!(
                        name,
                        "allMissingReturns" | "positionsBeforeReturns" | "unevenRawCounts"
                    ) {
                        assert_eq!(
                            events.len(),
                            missing.len() + 1,
                            "{profile}/{name}: {error:?}"
                        );
                        for ((reason, at), missing) in events.iter().zip(missing) {
                            assert_eq!(*reason, SemanticFailureReason::UnresolvedSymbol.into());
                            assert_eq!(
                                *at,
                                Some(position(SOURCE, offset(missing))),
                                "{profile}/{name}: {error:?}"
                            );
                        }
                        assert_eq!(
                            events.last().unwrap().0,
                            SemanticFailureReason::OverloadRecovery.into()
                        );
                    }
                    if missing.is_empty() {
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::OverloadRecovery.into(),
                            "{profile}/{name}: {error:?}"
                        );
                    }
                    assert!(
                        error.omitted_causes.is_none(),
                        "{profile}/{name}: {error:?}"
                    );
                }
            }
        }
    }
}

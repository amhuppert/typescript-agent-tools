//! Actual assignment producers continue through recovery before final admission.
//! The maintained native observer proves the within-query reduction cache hit.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{
    AnalysisError, SemanticError, SemanticErrorKind, SemanticFailureReason,
};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-assignment-continuation/cases.ts");
const PROFILES: [(&str, &str); 3] = [
    (
        "tsconfig",
        include_str!("../fixtures/types/native-assignment-continuation/expected-tsconfig.json"),
    ),
    (
        "loose",
        include_str!("../fixtures/types/native-assignment-continuation/expected-loose.json"),
    ),
    (
        "unchecked",
        include_str!("../fixtures/types/native-assignment-continuation/expected-unchecked.json"),
    ),
];

fn fixture() -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-assignment-continuation"),
    )
}

fn request(fixture: &Fixture, name: &str, expand: bool) -> TypeRequest {
    let marker = format!("/*#{name}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len()),
        },
        expand,
        depth: 3,
    }
}

fn semantic(answer: Result<TypeAnswer, AnalysisError>, label: &str) -> SemanticError {
    let Err(AnalysisError::Semantic(error)) = answer else {
        panic!("{label}: {answer:?}");
    };
    *error
}

#[test]
fn native_assignment_identity_and_refinement_controls_remain_distinct() {
    let fixture = fixture();
    for (profile, baseline) in PROFILES {
        let native: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(native.len(), 29);
        let project = fixture.load_config(&format!("{profile}.json"));
        for name in [
            "propertyHealthy",
            "tupleHealthy",
            "tupleDefaultHealthy",
            "tupleRestElement",
            "arrayLikeProperty",
            "actualAny",
            "assignedNever",
            "assignedIdentity",
            "incompatibleAssignment",
            "freshBoolean",
            "regularBoolean",
            "scalarSkipsAssignment",
            "forInAssignment",
            "forOfAssignment",
            "bindingInitialHealthy",
            "healthyReductionWarm",
        ] {
            let row = native.iter().find(|row| row["name"] == name).unwrap();
            assert_eq!(row["errorType"], false);
            let changes_read = matches!(
                name,
                "assignedNever" | "freshBoolean" | "regularBoolean" | "forInAssignment"
            );
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, name, expand));
                if changes_read {
                    let error = semantic(answer, &format!("{profile}/{name}/{expand}"));
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::UnsupportedSemantic,
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(
                        error.reason,
                        SemanticFailureReason::FlowRequired.into(),
                        "{profile}/{name}: {error:?}"
                    );
                    assert!(error.causes.is_empty(), "{profile}/{name}: {error:?}");
                    continue;
                }
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
                if expand {
                    assert!(
                        result
                            .description
                            .expansion
                            .as_ref()
                            .unwrap()
                            .nodes
                            .iter()
                            .all(|node| !matches!(node.data, TypeNodeData::Unsupported { .. })),
                        "{profile}/{name}: {result:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn recovered_projection_keeps_the_later_default_and_native_nested_skip() {
    let fixture = fixture();
    let file = fixture.path("cases.ts").canonicalize().unwrap();
    let missing = SemanticFailureReason::UnresolvedSymbol;
    let error_type = SemanticFailureReason::ErrorType;
    for (profile, _) in PROFILES {
        let project = fixture.load_config(&format!("{profile}.json"));
        for (name, first, first_reason) in [
            ("propertyRecovery", "MissingPropertyProjection", missing),
            ("indexRecovery", "MissingIndexProjection", missing),
            ("absentRecovery", "absent: value = later.value", error_type),
            (
                "unusableKeyRecovery",
                "[badKey]: value = later.value",
                error_type,
            ),
            ("shorthandRecovery", "MissingPropertyProjection", missing),
            ("tupleRecovery", "MissingTupleProjection", missing),
            // Native's isDestructuringAssignmentTarget only checks the immediate
            // container; this nested binary is ordinary RHS, not a default read.
            ("nestedRecovery", "MissingLaterDefault", missing),
            ("tupleOutOfBounds", "MissingLaterDefault", missing),
            ("iterationRecovery", "MissingIterationProjection", missing),
            ("bindingInitialRecovery", "MissingTupleProjection", missing),
        ] {
            let first_at = position(SOURCE, SOURCE.find(first).unwrap());
            let later_at = position(SOURCE, SOURCE.find("MissingLaterDefault").unwrap());
            for expand in [false, true] {
                let error = semantic(
                    project.type_of(request(&fixture, name, expand)),
                    &format!("{profile}/{name}/{expand}"),
                );
                assert_eq!(
                    error.kind,
                    SemanticErrorKind::InvalidSemantic,
                    "{profile}/{name}: {error:?}"
                );
                assert_eq!(
                    error.reason,
                    first_reason.into(),
                    "{profile}/{name}: {error:?}"
                );
                assert_eq!(error.path.as_deref(), Some(file.as_path()));
                assert_eq!(
                    error.position,
                    Some(first_at),
                    "{profile}/{name}: {error:?}"
                );
                let mut saw_later = first_at == later_at;
                for cause in &error.causes {
                    let at = cause
                        .location
                        .as_ref()
                        .expect("assignment acquisition has an actual site");
                    assert_eq!(at.file, "cases.ts");
                    assert!(
                        at.start == first_at || at.start == later_at,
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(
                        cause.reason,
                        if at.start == first_at {
                            first_reason
                        } else {
                            missing
                        }
                        .into(),
                        "{profile}/{name}: {error:?}"
                    );
                    saw_later |= at.start == later_at;
                }
                assert!(
                    saw_later,
                    "later native default was lost: {profile}/{name}: {error:?}"
                );
                assert!(
                    error.omitted_causes.is_none(),
                    "{profile}/{name}: {error:?}"
                );
            }
        }
    }
}

#[test]
fn actual_same_pair_cache_hits_replay_completed_assignment_evidence() {
    let fixture = fixture();
    let file = fixture.path("cases.ts").canonicalize().unwrap();
    let left = position(SOURCE, SOURCE.find("MissingLeftComparison").unwrap());
    let right = position(SOURCE, SOURCE.find("MissingRightComparison").unwrap());
    for (profile, baseline) in PROFILES {
        let native: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        for name in [
            "reductionWarm",
            "reductionWarmReverse",
            "healthyReductionWarm",
        ] {
            let row = native.iter().find(|row| row["name"] == name).unwrap();
            let events = row["assignments"].as_array().unwrap();
            assert_eq!(events.len(), 3);
            assert_eq!(events[0]["hit"], false);
            assert_eq!(events[1]["event"], "installed");
            assert_eq!(events[2]["hit"], true);
            assert_eq!(events[0]["declared"], events[2]["declared"]);
            assert_eq!(events[0]["assigned"], events[2]["assigned"]);
        }
        let project = fixture.load_config(&format!("{profile}.json"));
        for expand in [false, true] {
            let mut cold = None;
            for name in ["reductionCold", "reductionWarm", "reductionWarmReverse"] {
                let error = semantic(
                    project.type_of(request(&fixture, name, expand)),
                    &format!("{profile}/{name}/{expand}"),
                );
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
                assert_eq!(error.path.as_deref(), Some(file.as_path()));
                assert_eq!(error.position, Some(left), "{profile}/{name}: {error:?}");
                let mut events = vec![left];
                for cause in &error.causes {
                    assert_eq!(
                        cause.reason,
                        SemanticFailureReason::UnresolvedSymbol.into(),
                        "{profile}/{name}: {error:?}"
                    );
                    let at = cause.location.as_ref().unwrap();
                    assert_eq!(at.file, "cases.ts");
                    assert!(
                        at.start == left || at.start == right,
                        "{profile}/{name}: {error:?}"
                    );
                    events.push(at.start);
                }
                assert!(
                    events.contains(&right),
                    "right comparison was lost: {profile}/{name}: {error:?}"
                );
                assert!(
                    error.omitted_causes.is_none(),
                    "{profile}/{name}: {error:?}"
                );
                if name == "reductionCold" {
                    cold = Some(events);
                } else {
                    // Native proves one computation followed by one hit in the
                    // same checker. Each read consumes the installed completion.
                    let cold = cold.as_ref().unwrap();
                    let expected: Vec<_> = cold.iter().chain(cold).copied().collect();
                    assert_eq!(events, expected, "{profile}/{name}: {error:?}");
                }
            }
        }
    }
}

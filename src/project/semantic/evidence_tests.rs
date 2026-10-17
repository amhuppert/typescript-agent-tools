//! Recovery evidence remains typed across internal conversions and output caps.
use super::*;
use crate::source::SourcePosition;

fn location(column: usize) -> Location {
    Location {
        file: "cases.ts".into(),
        start: SourcePosition { line: 1, column },
        end: SourcePosition {
            line: 1,
            column: column + 1,
        },
    }
}
fn failure(kind: SemanticErrorKind, message: &str) -> model::DemandError {
    AnalysisError::from(SemanticError {
        kind,
        reason: match kind {
            SemanticErrorKind::SemanticDepthLimit => {
                SemanticResourceReason::SemanticDepthLimit.into()
            }
            SemanticErrorKind::InvalidSemantic => SemanticFailureReason::ErrorType.into(),
            _ => SemanticFailureReason::UnsupportedSemanticForm.into(),
        },
        message: message.into(),
        path: Some("/project/cases.ts".into()),
        position: Some(location(1).start),
        exact_location: Some(location(1)),
        unbound_recovery: None,
        causes: Vec::new(),
        omitted_causes: None,
        cause_kinds: Default::default(),
    })
    .into()
}
fn semantic(failure: model::DemandError) -> SemanticError {
    let AnalysisError::Semantic(error) = AnalysisError::from(failure) else {
        panic!("semantic evidence expected")
    };
    *error
}

#[test]
fn located_primary_and_repeated_unbound_children_survive_conversion_and_replay() {
    let unknown =
        || unbound_recovery(crate::analysis::UnboundSemanticRecovery::UnknownSignatureReturn);
    let cached = failure(SemanticErrorKind::InvalidSemantic, "first")
        .append_failure(unknown())
        .append_failure(unknown());
    for column in [3, 9] {
        // Q4 currently passes through AnalysisError before its outer consumer
        // observes the result. The secondary markers must survive that route.
        let converted: model::DemandError = AnalysisError::from(cached.clone()).into();
        let mut observed = semantic(converted);
        observed.observe_recovery(&location(column), || {
            panic!("located primary must retain its own path")
        });
        assert_eq!(observed.position, Some(location(1).start));
        assert_eq!(observed.causes.len(), 2);
        for cause in &observed.causes {
            assert_eq!(cause.location, Some(location(column)));
            assert_eq!(cause.kind, Some(SemanticErrorKind::InvalidSemantic));
            assert_eq!(cause.unbound_recovery, None);
        }
    }
    let cached = semantic(cached);
    assert!(cached.causes.iter().all(|cause| cause.location.is_none()
        && cause.unbound_recovery
            == Some(crate::analysis::UnboundSemanticRecovery::UnknownSignatureReturn)));
}

#[test]
fn omitted_invalid_dependencies_still_forbid_an_unsupported_leaf() {
    for invalid_index in [15, 16, 17] {
        let mut child = failure(SemanticErrorKind::UnsupportedSemantic, "child");
        for index in 0..18 {
            child = child.append_failure(failure(
                if index == invalid_index {
                    SemanticErrorKind::InvalidSemantic
                } else {
                    SemanticErrorKind::UnsupportedSemantic
                },
                &index.to_string(),
            ));
        }
        let child = semantic(child);
        assert_eq!(child.causes.len(), 16);
        assert_eq!(child.omitted_causes, Some(2));
        assert!(!child.permits_unsupported_leaf());
        let outer = failure(SemanticErrorKind::UnsupportedSemantic, "outer")
            .append_failure(AnalysisError::from(child).into());
        let outer = semantic(outer);
        assert_eq!(outer.causes[0].message, "child");
        assert_eq!(outer.causes[1].message, "0");
        assert_eq!(outer.causes[15].message, "14");
        assert_eq!(outer.omitted_causes, Some(3));
        assert!(!outer.permits_unsupported_leaf());
    }
    let supported = semantic(
        failure(SemanticErrorKind::UnsupportedSemantic, "first")
            .append_failure(failure(SemanticErrorKind::UnsupportedSemantic, "second")),
    );
    assert!(supported.permits_unsupported_leaf());
    // The wire schema stays unchanged; wire input lacks private evidence and
    // therefore cannot authorize dropping an acquired dependency into a leaf.
    let serialized = serde_json::to_value(&supported).unwrap();
    assert!(serialized.get("causeKinds").is_none());
    assert!(serialized["causes"][0].get("kind").is_none());
    assert!(
        !serde_json::from_value::<SemanticError>(serialized)
            .unwrap()
            .permits_unsupported_leaf()
    );
}

#[test]
fn whole_failure_replaces_evidence_and_stops_later_aggregation() {
    let before = failure(SemanticErrorKind::InvalidSemantic, "before");
    let whole = failure(SemanticErrorKind::SemanticDepthLimit, "depth limit");
    let after = failure(SemanticErrorKind::InvalidSemantic, "after");
    let error = semantic(before.append_failure(whole).append_failure(after));
    assert_eq!(error.kind, SemanticErrorKind::SemanticDepthLimit);
    assert_eq!(error.message, "depth limit");
    assert!(error.causes.is_empty());
}

#[test]
fn cause_prefix_limit_preserves_under_at_and_over_boundaries() {
    for count in [15, 16, 17] {
        let mut error = failure(SemanticErrorKind::UnsupportedSemantic, "first");
        for index in 0..count {
            error = error.append_failure(failure(
                SemanticErrorKind::UnsupportedSemantic,
                &index.to_string(),
            ));
        }
        let error = semantic(error);
        assert_eq!(error.causes.len(), count.min(16));
        assert_eq!(
            error.omitted_causes,
            if count > 16 { Some(count - 16) } else { None }
        );
        assert!(error.permits_unsupported_leaf());
        assert_eq!(
            error.causes.last().unwrap().message,
            (count.min(16) - 1).to_string()
        );
    }
}

#[test]
fn appending_a_known_cause_does_not_qualify_an_unknown_omitted_suffix() {
    let original = semantic(failure(SemanticErrorKind::UnsupportedSemantic, "first"));
    let mut wire = serde_json::to_value(original).unwrap();
    wire["omittedCauses"] = 1.into();
    let mut restored: SemanticError = serde_json::from_value(wire).unwrap();
    assert!(restored.causes.is_empty());
    assert!(!restored.permits_unsupported_leaf());
    let known = semantic(
        failure(SemanticErrorKind::UnsupportedSemantic, "later")
            .append_failure(failure(SemanticErrorKind::UnsupportedSemantic, "child")),
    );
    let mut nested = restored.clone();
    nested.append_dependency(&known).unwrap();
    assert_eq!(nested.causes.len(), 2);
    assert_eq!(nested.omitted_causes, Some(1));
    assert!(!nested.permits_unsupported_leaf());
    restored.append_cause(known.as_cause()).unwrap();
    assert_eq!(restored.causes.len(), 1);
    assert_eq!(restored.omitted_causes, Some(1));
    assert!(!restored.permits_unsupported_leaf());
}

#[test]
fn native_completion_uses_actual_values_without_admitting_consumed_recovery() {
    use model::NativeCompletion;
    let completion =
        NativeCompletion::recovered(7, failure(SemanticErrorKind::InvalidSemantic, "first"))
            .unwrap_or_else(|_| panic!("native recovery value must complete"));
    let mut visited = Vec::new();
    let result = completion
        .and_then(|actual| {
            visited.push(actual);
            NativeCompletion::recovered(
                actual + 2,
                failure(SemanticErrorKind::InvalidSemantic, "second"),
            )
        })
        .and_then(|completion| {
            completion.and_then_value(|actual| {
                visited.push(actual);
                Ok(1)
            })
        })
        .and_then(NativeCompletion::admit);
    assert_eq!(visited, vec![7, 9]);
    let Err(error) = result else {
        panic!("healthy final value must retain consumed recovery")
    };
    let error = semantic(error);
    assert_eq!(error.message, "first");
    assert_eq!(error.causes.len(), 1);
    assert_eq!(error.causes[0].message, "second");
}

#[test]
fn native_completion_interruption_stops_later_work_and_preserves_whole_failure() {
    use model::NativeCompletion;
    for kind in [
        SemanticErrorKind::UnsupportedSemantic,
        SemanticErrorKind::SemanticDepthLimit,
    ] {
        let completion =
            NativeCompletion::recovered(7, failure(SemanticErrorKind::InvalidSemantic, "first"))
                .unwrap_or_else(|_| panic!("native recovery value must complete"));
        let result = completion
            .and_then_value(|_| Err::<usize, _>(failure(kind, "interruption")))
            .and_then(|completion| {
                completion.and_then_value(|_| -> Result<usize, model::DemandError> {
                    panic!("an interrupted native computation must not acquire another dependency")
                })
            });
        let Err(error) = result else {
            panic!("interruption must remain outer Err")
        };
        let error = semantic(error);
        if kind == SemanticErrorKind::SemanticDepthLimit {
            assert_eq!(error.kind, kind);
            assert_eq!(error.message, "interruption");
            assert!(error.causes.is_empty());
        } else {
            assert_eq!(error.message, "first");
            assert_eq!(error.causes[0].message, "interruption");
        }
    }
}

#[test]
fn native_sequence_finishes_after_cleanup_without_replaying_prior_evidence() {
    use model::{NativeCompletion, NativeSequence};
    for stop in [
        None,
        Some(SemanticErrorKind::UnsupportedSemantic),
        Some(SemanticErrorKind::SemanticDepthLimit),
    ] {
        let mut sequence = NativeSequence::new();
        let mut visited = Vec::new();
        let result = (|| {
            let first = sequence.read(NativeCompletion::recovered(
                7,
                failure(SemanticErrorKind::InvalidSemantic, "first"),
            ))?;
            visited.push(first);
            let second = sequence.read(NativeCompletion::recovered(
                9,
                failure(SemanticErrorKind::InvalidSemantic, "second"),
            ))?;
            visited.push(second);
            if let Some(kind) = stop {
                sequence.read(Err::<NativeCompletion<()>, _>(failure(kind, "stop")))?;
                panic!("interrupted sequence continued");
            }
            Ok::<_, model::DemandError>(42)
        })();
        visited.push(99); // Native frame cleanup runs before finish/admission.
        let result = sequence.finish(result).and_then(NativeCompletion::admit);
        assert_eq!(visited, [7, 9, 99]);
        let error = semantic(result.expect_err("consumed recovery must fail admission"));
        if stop == Some(SemanticErrorKind::SemanticDepthLimit) {
            assert_eq!(error.message, "stop");
            assert!(error.causes.is_empty());
        } else {
            assert_eq!(error.message, "first");
            let causes: Vec<_> = error
                .causes
                .iter()
                .map(|cause| cause.message.as_str())
                .collect();
            assert_eq!(
                causes,
                if stop.is_some() {
                    vec!["second", "stop"]
                } else {
                    vec!["second"]
                }
            );
        }
    }
}

#[test]
fn relation_limit_markers_keep_prior_acquisition_order_and_bind_only_on_replay() {
    use crate::analysis::UnboundSemanticRecovery;
    use model::NativeCompletion;
    for (kind, reason) in [
        (
            UnboundSemanticRecovery::RelationDepth,
            SemanticFailureReason::RelationDepthLimit,
        ),
        (
            UnboundSemanticRecovery::RelationComplexity,
            SemanticFailureReason::RelationComplexityLimit,
        ),
    ] {
        let first = NativeCompletion::recovered(
            (),
            failure(SemanticErrorKind::InvalidSemantic, "earlier property"),
        )
        .unwrap_or_else(|_| panic!("native recovered value must complete"));
        let installed = first
            .and_then(|()| NativeCompletion::recovered(false, unbound_recovery(kind)))
            .unwrap_or_else(|_| panic!("native recovered value must complete"));
        for column in [3, 9] {
            let returned = installed
                .clone()
                .and_then(|value| {
                    NativeCompletion::recovered(
                        value,
                        failure(SemanticErrorKind::InvalidSemantic, "later display"),
                    )
                })
                .unwrap_or_else(|_| panic!("native recovered value must complete"));
            assert!(!returned.value());
            let mut observed = semantic(returned.admit().unwrap_err());
            observed.observe_recovery(&location(column), || {
                panic!("located primary must retain its path")
            });
            assert_eq!(observed.message, "earlier property");
            assert_eq!(observed.exact_location, Some(location(1)));
            assert_eq!(observed.causes.len(), 2);
            assert_eq!(observed.causes[0].reason, reason.into());
            assert_eq!(observed.causes[0].location, Some(location(column)));
            assert_eq!(observed.causes[1].message, "later display");
            assert_eq!(observed.causes[1].location, Some(location(1)));
        }
        let cached = semantic(installed.admit().unwrap_err());
        assert_eq!(cached.causes.len(), 1);
        assert_eq!(cached.causes[0].location, None);
        assert_eq!(cached.causes[0].unbound_recovery, Some(kind));
    }
}

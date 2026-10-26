//! Supplemental field preservation; final-query fees live in completed_return.
use super::*;

#[test]
fn request_resource_completed_copy_preserves_private_and_public_evidence() {
    use crate::analysis::{SemanticCause, UnboundSemanticRecovery};
    use model::{NativeCompletion, PropertySymbolId, PropertyValueQuestion};

    for count in [0, 15, 16, 17] {
        for primary_kind in [
            SemanticErrorKind::UnsupportedSemantic,
            SemanticErrorKind::InvalidSemantic,
        ] {
            for question in [
                PropertyValueQuestion::Read(PropertySymbolId::new(7)),
                PropertyValueQuestion::Write(PropertySymbolId::new(7)),
            ] {
                let location = Location {
                    file: "Σ𝒜.ts".into(),
                    start: SourcePosition { line: 2, column: 3 },
                    end: SourcePosition { line: 2, column: 5 },
                };
                let mut error = SemanticError {
                    kind: primary_kind,
                    reason: if primary_kind == SemanticErrorKind::UnsupportedSemantic {
                        SemanticFailureReason::UnsupportedSemanticForm
                    } else {
                        SemanticFailureReason::ErrorType
                    }
                    .into(),
                    message: "primary_Σ𝒜".into(),
                    path: Some("/captured/Σ𝒜.ts".into()),
                    position: Some(location.start),
                    exact_location: Some(location.clone()),
                    unbound_recovery: None,
                    causes: (0..count)
                        .map(|index| SemanticCause {
                            reason: if index == 16 {
                                SemanticFailureReason::ErrorType
                            } else {
                                SemanticFailureReason::UnsupportedSemanticForm
                            }
                            .into(),
                            location: (index % 2 == 0).then(|| location.clone()),
                            message: format!("cause_{index}_Σ𝒜"),
                            kind: Some(if index == 16 {
                                SemanticErrorKind::InvalidSemantic
                            } else {
                                SemanticErrorKind::UnsupportedSemantic
                            }),
                            unbound_recovery: (index % 2 != 0)
                                .then_some(UnboundSemanticRecovery::UnknownSignatureReturn),
                        })
                        .collect(),
                    omitted_causes: None,
                    cause_kinds: Default::default(),
                };
                error.bound_causes().unwrap();
                let eligibility = error.permits_unsupported_leaf();
                let expected: AnalysisError = error.into();
                let issue = (primary_kind == SemanticErrorKind::UnsupportedSemantic).then(|| {
                    crate::types::TypeIssue {
                        reason: SemanticFailureReason::UnsupportedSemanticForm,
                        location,
                        message: "issue_Σ𝒜".into(),
                    }
                });
                let cached = match NativeCompletion::recovered(
                    42usize,
                    model::DemandError::new(expected.clone(), issue.clone()).at_property(question),
                ) {
                    Ok(value) => value,
                    Err(_) => panic!("recoverable evidence rejected"),
                };
                for _ in 0..2 {
                    let copied = cached.clone();
                    assert_eq!(*copied.value(), 42);
                    let mut failure = match copied.admit() {
                        Err(failure) => failure,
                        Ok(_) => panic!("copied recovery became an admitted value"),
                    };
                    assert!(
                        failure.failed_property == Some(question),
                        "failed property identity changed"
                    );
                    assert_eq!(failure.take_issue(), issue);
                    let AnalysisError::Semantic(actual) = failure.error() else {
                        panic!("semantic classification lost");
                    };
                    assert_eq!(actual.permits_unsupported_leaf(), eligibility);
                    assert_eq!(AnalysisError::from(failure), expected);
                }
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn request_resource_completed_copy_preserves_exact_os_path() {
    use std::os::unix::ffi::OsStringExt;
    let fixture = Fixture::new("let chosen: number = 1;");
    let result: Result<(), AnalysisError> =
        worker_with_limits(&fixture.project, Limits::default(), |_, _| {
            let expected: AnalysisError = SemanticError {
                kind: SemanticErrorKind::InvalidSemantic,
                reason: SemanticFailureReason::UnresolvedSymbol.into(),
                message: "retained identity".into(),
                path: Some(std::ffi::OsString::from_vec(b"/captured/\xff.ts".to_vec()).into()),
                position: None,
                exact_location: None,
                unbound_recovery: None,
                causes: Vec::new(),
                omitted_causes: None,
                cause_kinds: Default::default(),
            }
            .into();
            let cached = match model::NativeCompletion::recovered(1usize, expected.clone().into()) {
                Ok(value) => value,
                Err(_) => panic!("recoverable evidence rejected"),
            };
            let copied = match cached.clone().admit() {
                Err(failure) => AnalysisError::from(failure),
                Ok(_) => panic!("copied recovery became success"),
            };
            assert_eq!(copied, expected);
            let AnalysisError::Semantic(error) = copied else {
                panic!("semantic classification lost");
            };
            assert_eq!(
                error.path.unwrap().as_os_str().as_encoded_bytes(),
                b"/captured/\xff.ts"
            );
            Ok(())
        });
    result.unwrap();
}

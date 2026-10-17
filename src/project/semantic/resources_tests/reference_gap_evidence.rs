//! Supplemental error-transport invariants; final-query accounting lives in reference_gaps.
use super::*;
use crate::analysis::{SemanticCause, UnboundSemanticRecovery};

fn failure(message: String, kind: SemanticErrorKind) -> SemanticError {
    SemanticError {
        kind,
        reason: if kind == SemanticErrorKind::UnsupportedSemantic {
            SemanticFailureReason::UnsupportedSemanticForm
        } else {
            SemanticFailureReason::UnresolvedSymbol
        }
        .into(),
        message,
        path: Some(PathBuf::from("/captured/Σ𝒜.ts")),
        position: Some(SourcePosition { line: 2, column: 3 }),
        exact_location: Some(Location {
            file: "Σ𝒜.ts".into(),
            start: SourcePosition { line: 2, column: 3 },
            end: SourcePosition { line: 2, column: 5 },
        }),
        unbound_recovery: Some(UnboundSemanticRecovery::UnknownSignatureReturn),
        causes: Vec::new(),
        omitted_causes: None,
        cause_kinds: Default::default(),
    }
}

#[test]
fn request_resource_error_copy_preserves_provenance_and_classification() {
    for count in [15, 16, 17] {
        let mut original = failure("primary_Σ𝒜".into(), SemanticErrorKind::UnsupportedSemantic);
        for i in 0..count {
            original.causes.push(SemanticCause {
                reason: if i == 16 {
                    SemanticFailureReason::UnresolvedSymbol
                } else {
                    SemanticFailureReason::UnsupportedSemanticForm
                }
                .into(),
                message: format!("cause_{i}_Σ𝒜"),
                location: original.exact_location.clone(),
                kind: Some(if i == 16 {
                    SemanticErrorKind::InvalidSemantic
                } else {
                    SemanticErrorKind::UnsupportedSemantic
                }),
                unbound_recovery: Some(UnboundSemanticRecovery::InstalledGlobalType),
            });
        }
        let mut copied = original.clone();
        assert_eq!(copied, original);
        // Classification happens before trimming the public prefix. The omitted
        // invalid dependency must still forbid an unsupported Q4 leaf.
        copied.bound_causes().unwrap();
        assert_eq!(copied.causes.len(), count.min(16));
        assert_eq!(
            copied.omitted_causes,
            (count > 16).then_some(count - count.min(16))
        );
        assert_eq!(copied.permits_unsupported_leaf(), count <= 16);
        for (i, cause) in copied.causes.iter().enumerate() {
            assert_eq!(cause.message, format!("cause_{i}_Σ𝒜"));
            assert_eq!(
                cause.unbound_recovery,
                Some(UnboundSemanticRecovery::InstalledGlobalType)
            );
        }
        let roundtrip: SemanticError =
            serde_json::from_value(serde_json::to_value(&copied).unwrap()).unwrap();
        assert!(!roundtrip.permits_unsupported_leaf());
    }
}

#[test]
fn request_resource_error_cause_overflow_stays_an_output_failure() {
    let mut primary = failure("primary".into(), SemanticErrorKind::UnsupportedSemantic);
    primary.omitted_causes = Some(usize::MAX);
    let mut dependency = failure("dependency".into(), SemanticErrorKind::UnsupportedSemantic);
    dependency.omitted_causes = Some(1);
    let Err(AnalysisError::Semantic(error)) = primary.append_dependency(&dependency) else {
        panic!("cause overflow became success or changed error family")
    };
    assert_eq!(error.kind, SemanticErrorKind::OutputLimit);
    assert_eq!(error.reason, SemanticResourceReason::OutputLimit.into());
    assert!(error.path.is_none());
    assert!(error.position.is_none());
    assert!(error.causes.is_empty());
    assert!(error.omitted_causes.is_none());
}

#[cfg(unix)]
#[test]
fn request_resource_error_copy_keeps_exact_os_path_bytes() {
    use std::os::unix::ffi::OsStringExt;

    let mut original = failure("source".into(), SemanticErrorKind::InvalidSemantic);
    original.path = Some(std::ffi::OsString::from_vec(b"/captured/\xff.ts".to_vec()).into());
    let copied = original.clone();
    assert_eq!(copied, original);
    assert_eq!(
        copied.path.unwrap().as_os_str().as_encoded_bytes(),
        b"/captured/\xff.ts"
    );
}

//! Contextual union selection preserves completed source recovery before later
//! method parameter acquisition, including native context-free cache reuse.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn contextual_discriminants_preserve_selection_and_later_recovery() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-contextual-discriminants");
    let source = std::fs::read_to_string(root.join("cases.ts")).unwrap();
    for profile in ["strict", "loose", "exact", "nocheck"] {
        let config = std::fs::read_to_string(root.join(format!("{profile}.json"))).unwrap();
        let expected: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(expected.len(), 19);
        let fixture = Fixture::new(&[("tsconfig.json", &config), ("cases.ts", &source)]);
        for reverse in [false, true] {
            let project = fixture.load();
            let mut rows: Vec<_> = expected.iter().collect();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let at = position(&source, source.find(&marker).unwrap() + marker.len());
                let recovered: &[&str] = match name {
                    "unknownTag" => &["missingTag"],
                    "broken" => &["missingBrokenTag", "MissingFirst", "MissingSecond"],
                    "shorthand" => &["MissingShorthand", "MissingFirst", "MissingSecond"],
                    _ => &[],
                };
                assert_eq!(
                    row["errors"].as_array().unwrap().is_empty(),
                    recovered.is_empty()
                );
                for expand in [false, true, false] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: at,
                        },
                        expand,
                        depth: 0,
                    });
                    if let Some((first, later)) = recovered.split_first() {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::InvalidSemantic,
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.position,
                            Some(position(&source, source.find(first).unwrap())),
                            "{label}: {error:?}"
                        );
                        for token in later {
                            let expected_at = position(&source, source.find(token).unwrap());
                            assert!(
                                error.causes.iter().any(|cause| cause
                                    .location
                                    .as_ref()
                                    .is_some_and(|location| location.start == expected_at)
                                    && cause.reason
                                        == SemanticFailureReason::UnresolvedSymbol.into()),
                                "{label}: missing later {token}: {error:?}"
                            );
                        }
                    } else {
                        assert_eq!(row["errorType"], false);
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{label}");
                        assert_eq!(
                            answer.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{label}"
                        );
                        assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                    }
                }
            }
        }
    }
}

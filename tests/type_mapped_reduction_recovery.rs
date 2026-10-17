//! Homomorphic mapped instantiation carries intersection reduction recovery
//! through the later mapped template acquisition.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn mapped_reduction_preserves_later_template_recovery() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-mapped-reduction-recovery");
    let source = std::fs::read_to_string(root.join("cases.ts")).unwrap();
    for profile in ["strict", "loose", "nocheck"] {
        let config = std::fs::read_to_string(root.join(format!("{profile}.json"))).unwrap();
        let expected: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(expected.len(), 6);
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
                    "homomorphic" => &["MissingReduction", "MissingMappedTemplate"],
                    "remapped" => &["MissingRemapReduction", "MissingRemappedTemplate"],
                    "optional" => &["MissingOptionalReduction", "MissingOptionalTemplate"],
                    "union" => &["MissingUnionReduction", "MissingUnionTemplate"],
                    _ => &[],
                };
                let node_errors: Vec<_> = row["errors"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|event| event["operation"] == "getTypeFromTypeNode")
                    .map(|event| {
                        source[event["pos"].as_u64().unwrap() as usize
                            ..event["end"].as_u64().unwrap() as usize]
                            .trim()
                    })
                    .collect();
                assert_eq!(
                    node_errors, recovered,
                    "{profile}/{name}: native dependency order"
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
                            error.path,
                            Some(fixture.path("cases.ts")),
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
                    } else if name == "never" {
                        // Native reduction skips the unrelated payload and
                        // mapped template. Only the access on never recovers.
                        // Native reports the nonexistent property on its right/name node.
                        assert_eq!(row["errorType"], true);
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
                            SemanticFailureReason::ErrorType.into(),
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.path,
                            Some(fixture.path("cases.ts")),
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.position,
                            Some(position(
                                &source,
                                source.find("neverMapped.value").unwrap() + "neverMapped.".len()
                            )),
                            "{label}: {error:?}"
                        );
                        assert!(
                            error.causes.is_empty(),
                            "{label}: unrelated missing types were acquired: {error:?}"
                        );
                        assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
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
                        if let Some(graph) = answer.description.expansion {
                            assert!(
                                graph.nodes.iter().all(|node| !matches!(
                                    node.data,
                                    TypeNodeData::Unsupported { .. }
                                )),
                                "{label}: {graph:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

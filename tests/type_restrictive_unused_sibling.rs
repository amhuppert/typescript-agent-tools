//! Restrictive self-cache hits retain the completed instantiation they return.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn restrictive_self_marker_preserves_unused_sibling_recovery() {
    let source = include_str!("../fixtures/types/native-restrictive-unused-sibling/cases.ts");
    let fixture = Fixture::new(&[
        ("cases.ts", source),
        (
            "strict.json",
            include_str!("../fixtures/types/native-restrictive-unused-sibling/strict.json"),
        ),
        (
            "nocheck.json",
            include_str!("../fixtures/types/native-restrictive-unused-sibling/nocheck.json"),
        ),
    ]);
    let file = fixture.path("cases.ts");
    let missing = source.find("MissingMarker").unwrap();
    let start = position(source, missing);
    let end = position(source, missing + "MissingMarker".len());
    for profile in ["strict", "nocheck"] {
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let names = if reverse {
                ["healthy", "query"]
            } else {
                ["query", "healthy"]
            };
            for name in names {
                let marker = format!("/*#{name}*/");
                let at = position(source, source.find(&marker).unwrap() + marker.len());
                for expand in [false, true, false] {
                    let label = format!("{profile}/{reverse}/{name}/{expand}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: file.clone(),
                            position: at,
                        },
                        expand,
                        depth: 1,
                    });
                    if name == "healthy" {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{label}: {answer:?}");
                        assert_eq!(answer.description.summary, "45", "{label}");
                        assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                        assert_eq!(answer.description.expansion.is_some(), expand, "{label}");
                        if let Some(graph) = &answer.description.expansion {
                            assert!(
                                graph.nodes.iter().all(|node| !matches!(
                                    node.data,
                                    TypeNodeData::Unsupported { .. }
                                )),
                                "{label}: {graph:?}"
                            );
                        }
                        continue;
                    }
                    // Native returns 41, but it consumes MissingMarker as a
                    // symbol while filtering the unused sibling's outer T.
                    // That completed filter has two acquisitions. Annotation
                    // instantiation reads it twice (4). The true branch has
                    // four; simplification adds the restrictive hit's two;
                    // the default constraint replays the true branch's four.
                    // Identifier narrowing, non-null facts, and apparent type
                    // each acquire that completed base: 4 + 3 * 10 = 34.
                    // An unconditional self-marker erases only the hit's two,
                    // giving 4 + 3 * 8 = 28 (registered guard-only proof).
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
                    assert_eq!(error.path.as_ref(), Some(&file), "{label}: {error:?}");
                    assert_eq!(error.position, Some(start), "{label}: {error:?}");
                    assert_eq!(
                        1 + error.causes.len() + error.omitted_causes.unwrap_or(0),
                        34,
                        "{label}: {error:?}"
                    );
                    assert_eq!(error.causes.len(), 16, "{label}: {error:?}");
                    assert_eq!(error.omitted_causes, Some(17), "{label}: {error:?}");
                    for cause in &error.causes {
                        assert_eq!(
                            cause.reason,
                            SemanticFailureReason::UnresolvedSymbol.into(),
                            "{label}: {error:?}"
                        );
                        let location = cause.location.as_ref().expect("located native acquisition");
                        assert_eq!(location.file, "cases.ts", "{label}: {error:?}");
                        assert_eq!(
                            (location.start, location.end),
                            (start, end),
                            "{label}: {error:?}"
                        );
                    }
                }
            }
        }
    }
}

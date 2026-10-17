//! Template matching keeps native continuation values and ordered recovery.
//! Native acquisition observations define required dependencies, not cause counts.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

fn dependencies(name: &str) -> &'static [&'static str] {
    match name {
        "equalSpans" => &["MissingEqualSource", "MissingEqualTarget"],
        "equalSpansTwo" => &[
            "MissingFirstSource",
            "MissingFirstTarget",
            "MissingSecondSource",
            "MissingSecondTarget",
        ],
        "crossingSpans" => &["MissingCrossSource", "MissingCrossTarget"],
        "inferSame" => &["MissingInferSource", "MissingInferTarget"],
        "inferCross" => &["MissingInferCrossTarget", "MissingInferCrossSource"],
        // Generic mapping observes only its source constraint in the cold native
        // run; it does not prove a later target acquisition.
        "mapping" => &["MissingMappingSource"],
        "equalReject" => &["MissingRejectedSource", "MissingRejectedTarget"],
        "validationContinue" => &["MissingReachedTarget"],
        "validationReject" => &["MissingSkippedTarget"],
        "mappingFixed"
        | "mappingChanges"
        | "mappingNestedFixed"
        | "mappingNestedChanges"
        | "healthySplit"
        | "healthy" => &[],
        _ => panic!("unclassified native row {name}"),
    }
}

fn check_rows(recovery: bool) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-template-completion");
    let source = std::fs::read_to_string(root.join("cases.ts")).unwrap();
    let strict = std::fs::read_to_string(root.join("strict.json")).unwrap();
    let loose = std::fs::read_to_string(root.join("loose.json")).unwrap();
    let nocheck = std::fs::read_to_string(root.join("nocheck.json")).unwrap();
    let fixture = Fixture::new(&[
        (
            "native-template-matching/globals.d.ts",
            include_str!("../fixtures/types/native-template-matching/globals.d.ts"),
        ),
        ("native-template-completion/cases.ts", &source),
        ("native-template-completion/strict.json", &strict),
        ("native-template-completion/loose.json", &loose),
        ("native-template-completion/nocheck.json", &nocheck),
    ]);
    for profile in ["strict", "loose", "nocheck"] {
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        let queries = expected["queries"].as_array().unwrap();
        assert_eq!(queries.len(), 15);
        for reverse in [false, true] {
            let project =
                fixture.load_config(&format!("native-template-completion/{profile}.json"));
            let mut rows: Vec<_> = queries
                .iter()
                .filter(|row| dependencies(row["name"].as_str().unwrap()).is_empty() != recovery)
                .collect();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let required = dependencies(name);
                assert_eq!(
                    row["demandFailures"],
                    serde_json::json!([]),
                    "{profile}/{name}"
                );
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert_eq!(
                    row["errors"].as_array().unwrap().is_empty(),
                    required.is_empty(),
                    "{profile}/{name}"
                );
                let mut native_events = row["errors"].as_array().unwrap().as_slice();
                for token in required {
                    let token_at = source.find(token).unwrap();
                    let found = native_events.iter().position(|event| {
                        event["operation"] == "getTypeFromTypeNode"
                            && event["pos"]
                                .as_u64()
                                .is_some_and(|pos| pos as usize <= token_at)
                            && event["end"]
                                .as_u64()
                                .is_some_and(|end| end as usize >= token_at + token.len())
                    });
                    let Some(found) = found else {
                        panic!("{profile}/{name}: native evidence lacks ordered {token}");
                    };
                    native_events = &native_events[found + 1..];
                }
                let marker = format!("/*#{name}*/");
                let at = position(&source, source.find(&marker).unwrap() + marker.len());
                for expand in [false, true, false] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("native-template-completion/cases.ts"),
                            position: at,
                        },
                        expand,
                        depth: 1,
                    });
                    if let Some((first, later)) = required.split_first() {
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
                            Some(fixture.path("native-template-completion/cases.ts")),
                            "{label}: {error:?}"
                        );
                        assert_eq!(
                            error.position,
                            Some(position(&source, source.find(first).unwrap())),
                            "{label}: {error:?}"
                        );
                        let mut remaining = error.causes.as_slice();
                        for token in later {
                            let expected_at = position(&source, source.find(token).unwrap());
                            let found = remaining.iter().position(|cause| {
                                cause.location.as_ref().is_some_and(|location| {
                                    location.file == "cases.ts" && location.start == expected_at
                                }) && cause.reason == SemanticFailureReason::UnresolvedSymbol.into()
                            });
                            let Some(found) = found else {
                                panic!("{label}: missing ordered {token}: {error:?}");
                            };
                            remaining = &remaining[found + 1..];
                        }
                    } else {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{label}: {answer:?}");
                        assert_eq!(
                            answer.description.summary,
                            row["summary"].as_str().unwrap(),
                            "{label}"
                        );
                        assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                        assert_eq!(
                            answer.description.expansion.is_some(),
                            expand,
                            "{label}: {answer:?}"
                        );
                        if let Some(graph) = &answer.description.expansion {
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

#[test]
fn template_matching_and_inference_preserve_ordered_recovery() {
    check_rows(true);
}

#[test]
fn template_and_mapping_short_circuits_preserve_native_results() {
    check_rows(false);
}

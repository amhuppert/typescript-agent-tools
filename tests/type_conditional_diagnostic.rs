//! Ordinary diagnostic conditional rendering retains branch acquisitions.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

fn branches(name: &str) -> &'static [&'static str] {
    match name {
        "inline" => &["MissingInlineTrue", "MissingInlineFalse"],
        "inferContext" => &["MissingInferTrue", "MissingInferFalse"],
        "nonDistributive" => &["MissingTupleTrue", "MissingTupleFalse"],
        "named" | "healthy" => &[],
        _ => panic!("unclassified native row {name}"),
    }
}

#[test]
fn conditional_diagnostic_rendering_keeps_later_branch_and_access_recovery() {
    check(false);
}

#[test]
fn named_diagnostic_elision_and_healthy_control_keep_native_demands() {
    check(true);
}

fn check(controls: bool) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-conditional-diagnostic");
    let source = std::fs::read_to_string(root.join("cases.ts")).unwrap();
    let strict = std::fs::read_to_string(root.join("strict.json")).unwrap();
    let loose = std::fs::read_to_string(root.join("loose.json")).unwrap();
    let nocheck = std::fs::read_to_string(root.join("nocheck.json")).unwrap();
    let fixture = Fixture::new(&[
        (
            "native-template-matching/globals.d.ts",
            include_str!("../fixtures/types/native-template-matching/globals.d.ts"),
        ),
        ("native-conditional-diagnostic/cases.ts", &source),
        ("native-conditional-diagnostic/strict.json", &strict),
        ("native-conditional-diagnostic/loose.json", &loose),
        ("native-conditional-diagnostic/nocheck.json", &nocheck),
    ]);
    let file = fixture.path("native-conditional-diagnostic/cases.ts");
    for profile in ["strict", "loose", "nocheck"] {
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        let queries = expected["queries"].as_array().unwrap();
        assert_eq!(queries.len(), 5);
        for reverse in [false, true] {
            let project =
                fixture.load_config(&format!("native-conditional-diagnostic/{profile}.json"));
            let mut rows: Vec<_> = queries
                .iter()
                .filter(|row| branches(row["name"].as_str().unwrap()).is_empty() == controls)
                .collect();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let required = branches(name);
                let marker = format!("/*#{name}*/");
                let query_at = source.find(&marker).unwrap() + marker.len();
                assert_eq!(
                    row["demandFailures"],
                    serde_json::json!([]),
                    "{profile}/{name}"
                );
                let native_errors = row["errors"].as_array().unwrap();
                let mut expected = Vec::new();
                if name == "healthy" {
                    assert_eq!(row["errorType"], false);
                    assert!(native_errors.is_empty());
                } else {
                    assert_eq!(row["errorType"], true);
                    assert_eq!(native_errors[0]["operation"], "getTypeOfSymbol");
                    let expression_at = query_at + source[query_at..].find("value.absent").unwrap();
                    // Native's non-value TypeLiteral symbol recovery is first;
                    // expression observation gives its previously absent site.
                    expected.push((
                        position(&source, expression_at),
                        SemanticFailureReason::ErrorType,
                    ));
                    let mut native_nodes: Vec<_> = native_errors
                        .iter()
                        .filter(|event| event["operation"] == "getTypeFromTypeNode")
                        .collect();
                    assert_eq!(native_nodes.len(), required.len(), "{profile}/{name}");
                    for (event, token) in native_nodes.drain(..).zip(required) {
                        let token_at = source.find(token).unwrap();
                        assert!(event["pos"].as_u64().unwrap() as usize <= token_at);
                        assert_eq!(
                            event["end"].as_u64().unwrap() as usize,
                            token_at + token.len()
                        );
                        expected.push((
                            position(&source, token_at),
                            SemanticFailureReason::UnresolvedSymbol,
                        ));
                    }
                    expected.push((
                        position(&source, expression_at + "value.".len()),
                        SemanticFailureReason::ErrorType,
                    ));
                }
                for expand in [false, true, false] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: file.clone(),
                            position: position(&source, query_at),
                        },
                        expand,
                        depth: 1,
                    });
                    let Some((primary, later)) = expected.split_first() else {
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
                        continue;
                    };
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{label}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{label}: {error:?}"
                    );
                    assert_eq!(error.reason, primary.1.into(), "{label}: {error:?}");
                    assert_eq!(error.position, Some(primary.0), "{label}: {error:?}");
                    assert_eq!(error.path.as_ref(), Some(&file), "{label}: {error:?}");
                    let mut remaining = error.causes.as_slice();
                    for (at, reason) in later {
                        let found = remaining.iter().position(|cause| {
                            cause.reason == (*reason).into()
                                && cause.location.as_ref().is_some_and(|location| {
                                    location.file == "cases.ts" && location.start == *at
                                })
                        });
                        let Some(found) = found else {
                            panic!("{label}: missing ordered {reason:?} at {at:?}: {error:?}");
                        };
                        remaining = &remaining[found + 1..];
                    }
                    assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
                    if name == "named" {
                        assert!(
                            error.causes.iter().all(
                                |cause| cause.reason == SemanticFailureReason::ErrorType.into()
                            ),
                            "{label}: {error:?}"
                        );
                    }
                }
            }
        }
    }
}

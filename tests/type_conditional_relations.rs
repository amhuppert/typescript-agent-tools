//! Conditional relation/inference continuations and actual merged generic alias
//! identity are native observations; product failures retain consumed recovery.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

fn dependencies(name: &str) -> &'static [&'static str] {
    match name {
        "targetBoth" => &["MissingTargetTrue", "MissingTargetFalse"],
        // Even the always-true target has both branches acquired by native
        // simplification before the later relation branch gates are applied.
        "targetTrue" => &["MissingTargetSelected", "MissingTargetUnselected"],
        "sourceBoth" => &["MissingSourceTrue", "MissingSourceFalse"],
        "sourceTarget" => &[
            "MissingComparedSourceTrue",
            "MissingComparedSourceFalse",
            "MissingComparedTargetTrue",
            "MissingComparedTargetFalse",
        ],
        "inferenceBoth" => &["MissingInferenceTrue", "MissingInferenceFalse"],
        "createdAliasValue"
        | "createdAliasRelation"
        | "createdAliasInference"
        | "ordinaryAliasRelation"
        | "ordinaryAliasInference"
        | "healthy" => &[],
        _ => panic!("unclassified native row {name}"),
    }
}

fn check_rows(recovery: bool) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-conditional-relations");
    let source = std::fs::read_to_string(root.join("cases.ts")).unwrap();
    let strict = std::fs::read_to_string(root.join("strict.json")).unwrap();
    let loose = std::fs::read_to_string(root.join("loose.json")).unwrap();
    let nocheck = std::fs::read_to_string(root.join("nocheck.json")).unwrap();
    let fixture = Fixture::new(&[
        (
            "native-variable-patterns/globals.d.ts",
            include_str!("../fixtures/types/native-variable-patterns/globals.d.ts"),
        ),
        ("native-conditional-relations/cases.ts", &source),
        ("native-conditional-relations/strict.json", &strict),
        ("native-conditional-relations/loose.json", &loose),
        ("native-conditional-relations/nocheck.json", &nocheck),
    ]);
    for profile in ["strict", "loose", "nocheck"] {
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        for flag in ["transient", "property", "typeAlias"] {
            assert_eq!(expected["alias"][flag], true, "{profile}/{flag}");
        }
        let queries = expected["queries"].as_array().unwrap();
        assert_eq!(queries.len(), 11);
        for reverse in [false, true] {
            let project =
                fixture.load_config(&format!("native-conditional-relations/{profile}.json"));
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
                let marker = format!("/*#{name}*/");
                let at = position(&source, source.find(&marker).unwrap() + marker.len());
                for expand in [false, true, false] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("native-conditional-relations/cases.ts"),
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
                            Some(fixture.path("native-conditional-relations/cases.ts")),
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
fn conditional_relations_and_inference_preserve_ordered_recovery() {
    check_rows(true);
}

#[test]
fn created_generic_aliases_retain_native_variance_and_inference() {
    check_rows(false);
}

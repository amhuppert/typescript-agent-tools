//! Native generated strings remain literal values; only observed ID digits are
//! outside the product's identity policy. Native context/width baselines prove
//! both reachability and the cold/warm difference that forbids guessed digits.
mod support;
use serde_json::{Value, json};
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

fn fixture() -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-generated-literal-context"),
    )
}

#[test]
fn generated_literal_values_use_exact_symbolic_output() {
    const SOURCE: &str =
        include_str!("../fixtures/types/native-generated-literal-context/cases.ts");
    let fixture = fixture();
    for mode in ["strict", "loose"] {
        let rows: Vec<Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-context-{mode}.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 12);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("context-{mode}.json"));
            let mut rows: Vec<_> = rows.iter().collect();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let at = position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len());
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: at,
                        },
                        expand,
                        depth: 0,
                    });
                    if name == "privateIdentityHead" {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{answer:?}");
                        };
                        assert_eq!(error.kind, SemanticErrorKind::UnsupportedSemantic);
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::UnrepresentableIdentity.into()
                        );
                        continue;
                    }
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{mode}/{name}/{expand}/{reverse}: {answer:?}");
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found);
                    assert!(answer.issues.is_empty());
                    assert!(!answer.description.summary_truncated);
                    let symbol = json!({"kind":"symbol", "ref":"s0"});
                    let (summary, parts) = match name {
                        "privateStatic" => (
                            "string(bytes(\"fe23\"), symbol(s0), \"@#field\")",
                            Some(vec![
                                json!({"kind":"bytes","value":"fe23"}),
                                symbol,
                                json!({"kind":"text","value":"@#field"}),
                            ]),
                        ),
                        "privateIdentity" => ("string(symbol(s0))", Some(vec![symbol])),
                        "privateConcat" => (
                            "string(bytes(\"7072656669783afe23\"), symbol(s0), \"@#field:suffix\")",
                            Some(vec![
                                json!({"kind":"bytes","value":"7072656669783afe23"}),
                                symbol,
                                json!({"kind":"text","value":"@#field:suffix"}),
                            ]),
                        ),
                        "privateUpper" => (
                            "string(\"�#\", symbol(s0), \"@#FIELD\")",
                            Some(vec![
                                json!({"kind":"text","value":"�#"}),
                                symbol,
                                json!({"kind":"text","value":"@#FIELD"}),
                            ]),
                        ),
                        "privateLower" | "privateCapitalize" => (
                            "string(\"�#\", symbol(s0), \"@#field\")",
                            Some(vec![
                                json!({"kind":"text","value":"�#"}),
                                symbol,
                                json!({"kind":"text","value":"@#field"}),
                            ]),
                        ),
                        "privateUncapitalize" => (
                            "string(\"�#\", symbol(s0), \"@#Field\")",
                            Some(vec![
                                json!({"kind":"text","value":"�#"}),
                                symbol,
                                json!({"kind":"text","value":"@#Field"}),
                            ]),
                        ),
                        _ => (row["summary"].as_str().unwrap(), None),
                    };
                    assert_eq!(
                        answer.description.summary, summary,
                        "{mode}/{name}/{expand}/{reverse}"
                    );
                    if let Some(graph) = answer.description.expansion {
                        assert!(!graph.truncated);
                        assert_eq!(graph.nodes.len(), 1);
                        assert!(!matches!(
                            graph.nodes[0].data,
                            TypeNodeData::Unsupported { .. }
                        ));
                        if let Some(parts) = parts {
                            let mut node = serde_json::to_value(&graph.nodes[0]).unwrap();
                            assert_eq!(node["kind"], "literal");
                            assert_eq!(node["base"], "string");
                            for part in node["value"]["parts"].as_array_mut().unwrap() {
                                if part["kind"] == "symbol" {
                                    let origins =
                                        part.as_object_mut().unwrap().remove("origins").unwrap();
                                    assert_eq!(origins.as_array().unwrap().len(), 1);
                                    assert_eq!(origins[0]["file"], "cases.ts");
                                    let class = SOURCE[..SOURCE.find(&marker).unwrap()]
                                        .rfind("class")
                                        .unwrap();
                                    assert_eq!(
                                        origins[0]["start"],
                                        serde_json::to_value(position(SOURCE, class)).unwrap()
                                    );
                                    assert_eq!(
                                        origins[0]["end"],
                                        serde_json::to_value(position(SOURCE, class + 5)).unwrap()
                                    );
                                }
                            }
                            assert_eq!(node["value"], json!({"parts":parts}), "{name}");
                        } else {
                            let node = serde_json::to_value(&graph.nodes[0]).unwrap();
                            if name == "privateInstance" {
                                assert_eq!(node["kind"], "any");
                            } else {
                                assert_eq!(node["kind"], "literal");
                                assert_eq!(
                                    node["value"],
                                    if name == "publicStatic" {
                                        "field"
                                    } else {
                                        "public"
                                    }
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn splitting_native_symbol_digits_is_explicit_in_both_width_histories() {
    let fixture = fixture();
    for group in ["cold", "warm"] {
        let file = format!("{group}.ts");
        let source = std::fs::read_to_string(fixture.path(&file)).unwrap();
        for mode in ["strict", "loose"] {
            let project = fixture.load_config(&format!("{group}-{mode}.json"));
            for name in ["whole", "split", "suffix", "split", "whole"] {
                let marker = format!("/*#{name}*/");
                let at = position(&source, source.find(&marker).unwrap() + marker.len());
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path(&file),
                            position: at,
                        },
                        expand,
                        depth: 1,
                    });
                    if name == "split" {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{answer:?}");
                        };
                        assert_eq!(error.kind, SemanticErrorKind::UnsupportedSemantic);
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::UnrepresentableIdentity.into()
                        );
                    } else {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found);
                        assert_eq!(answer.description.summary, "string | number");
                        assert!(answer.issues.is_empty());
                    }
                }
            }
        }
    }
}

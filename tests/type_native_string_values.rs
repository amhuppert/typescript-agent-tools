//! Scanner cooking, constant concatenation, and template normalization retain
//! distinct native byte values through the final type-query boundary.
mod support;
use serde_json::{Value, json};
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

fn fixture() -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-string-values"),
    )
}

fn request(fixture: &Fixture, file: &str, source: &str, name: &str, expand: bool) -> TypeRequest {
    let marker = format!("/*#{name}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path(file),
            position: position(source, source.find(&marker).unwrap() + marker.len()),
        },
        expand,
        depth: 2,
    }
}

fn bytes(hex: &str) -> Value {
    json!({"parts":[{"kind":"bytes", "value":hex}]})
}

#[test]
fn ordinary_source_and_enum_values_preserve_exact_native_strings() {
    let fixture = fixture();
    let source = std::fs::read_to_string(fixture.path("cases.ts")).unwrap();
    for profile in ["strict", "loose"] {
        let rows: Vec<Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 34);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered: Vec<_> = rows.iter().collect();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert_eq!(row["codes"], json!([]), "{profile}/{name}");
                assert_eq!(row["demandFailures"], json!([]), "{profile}/{name}");
                for expand in [false, true, true, false] {
                    let answer =
                        project.type_of(request(&fixture, "cases.ts", &source, name, expand));
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        answer.outcome,
                        TypeOutcome::Found,
                        "{profile}/{name}: {answer:?}"
                    );
                    assert_eq!(
                        answer.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}"
                    );
                    assert_eq!(
                        serde_json::to_value(&answer.target.location).unwrap(),
                        row["location"],
                        "{profile}/{name}"
                    );
                    assert!(answer.issues.is_empty(), "{profile}/{name}: {answer:?}");
                    assert!(!answer.description.summary_truncated);
                    let Some(graph) = answer.description.expansion else {
                        continue;
                    };
                    assert!(
                        graph
                            .nodes
                            .iter()
                            .all(|node| !matches!(node.data, TypeNodeData::Unsupported { .. })),
                        "{profile}/{name}: {graph:?}"
                    );
                    let expected = match name {
                        "highExpression" | "highDeclaration" | "highType" | "highTemplate"
                        | "enumHigh" | "enumHighTemplate" => Some(bytes("eda080")),
                        "lowExpression" | "enumLow" => Some(bytes("edb080")),
                        "reversedHalves" => Some(bytes("edb080eda080")),
                        "continuedLF" | "continuedCR" | "continuedCRLF" | "continuedLS"
                        | "continuedPS" | "constantTemplate" | "enumConcat" => {
                            Some(bytes("eda080edb080"))
                        }
                        "fixedFixed"
                        | "fixedBraced"
                        | "bracedFixed"
                        | "bracedBraced"
                        | "scalar"
                        | "typeTemplate"
                        | "continuedTypeTemplate"
                        | "enumPair"
                        | "enumConcatTemplate" => Some(json!("𐀀")),
                        "replacement" => Some(json!("�")),
                        _ => None,
                    };
                    if let Some(expected) = expected {
                        assert!(!graph.truncated, "{profile}/{name}: {graph:?}");
                        let root = graph
                            .nodes
                            .iter()
                            .find(|node| node.id == graph.root)
                            .unwrap();
                        let root = serde_json::to_value(root).unwrap();
                        assert_eq!(root["kind"], "literal", "{profile}/{name}: {root}");
                        assert_eq!(root["value"], expected, "{profile}/{name}");
                    }
                    let expected_elements = match name {
                        "mappings" => Some(vec![
                            bytes("eda08041"),
                            bytes("edb08061"),
                            bytes("eda08061"),
                            bytes("edb08041"),
                        ]),
                        "expansionAndSigma" => Some(vec![
                            bytes("eda0805353"),
                            bytes("61cf82eda080"),
                            bytes("eda080edb080"),
                        ]),
                        _ => None,
                    };
                    if let Some(expected) = expected_elements {
                        let root = graph
                            .nodes
                            .iter()
                            .find(|node| node.id == graph.root)
                            .unwrap();
                        let TypeNodeData::Tuple {
                            elements,
                            omitted_elements,
                            ..
                        } = &root.data
                        else {
                            panic!("{profile}/{name}: {root:?}");
                        };
                        assert!(omitted_elements.is_none());
                        let values: Vec<_> = elements
                            .iter()
                            .map(|element| {
                                let typescript_parser::types::TypeRef::Ref { r#ref } =
                                    &element.value_type
                                else {
                                    panic!("{profile}/{name}: {element:?}");
                                };
                                let node =
                                    graph.nodes.iter().find(|node| node.id == *r#ref).unwrap();
                                let node = serde_json::to_value(node).unwrap();
                                assert_eq!(node["kind"], "literal");
                                node["value"].clone()
                            })
                            .collect();
                        assert_eq!(values, expected, "{profile}/{name}: {graph:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn enum_value_diagnostics_preserve_actual_call_failure_and_do_not_require_scalar_text() {
    let fixture = fixture();
    let source = std::fs::read_to_string(fixture.path("diagnostics.ts")).unwrap();
    for profile in [
        "diagnostics-strict",
        "diagnostics-loose",
        "diagnostics-no-check-strict",
        "diagnostics-no-check-loose",
    ] {
        let rows: Vec<Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        let project = fixture.load_config(&format!("{profile}.json"));
        assert_eq!(rows.len(), 4);
        for row in rows {
            let name = row["name"].as_str().unwrap();
            assert_eq!(row["errorType"], false);
            assert_eq!(row["demandFailures"], json!([]));
            for expand in [false, true, true, false] {
                let answer =
                    project.type_of(request(&fixture, "diagnostics.ts", &source, name, expand));
                if name == "enumEqual" {
                    assert_eq!(row["codes"], json!([]));
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{profile}/{name}: {answer:?}");
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found, "{answer:?}");
                    assert_eq!(answer.description.summary, row["summary"].as_str().unwrap());
                    assert!(answer.issues.is_empty());
                } else {
                    assert_eq!(row["codes"], json!([2345]));
                    let marker = source.find(&format!("/*#{name}*/")).unwrap();
                    let call = source[..marker].rfind('\n').unwrap() + 1;
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}: {answer:?}");
                    };
                    assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic, "{error:?}");
                    assert_eq!(
                        error.reason,
                        SemanticFailureReason::OverloadRecovery.into(),
                        "{error:?}"
                    );
                    assert_eq!(error.path, Some(fixture.path("diagnostics.ts")));
                    assert_eq!(error.position, Some(position(&source, call)), "{error:?}");
                    assert!(error.causes.is_empty(), "{error:?}");
                    assert!(error.omitted_causes.is_none());
                }
            }
        }
    }
}

#[test]
fn typeof_switch_witnesses_compare_exact_cooked_bytes() {
    let fixture = fixture();
    let source = std::fs::read_to_string(fixture.path("switches.ts")).unwrap();
    for profile in ["strict", "loose"] {
        let rows: Vec<Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-switches-{profile}.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 10);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("switches-{profile}.json"));
            let mut rows: Vec<_> = rows.iter().collect();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                assert_eq!(row["codes"], json!([]));
                assert_eq!(row["demandFailures"], json!([]));
                for expand in [false, true, true, false] {
                    let answer =
                        project.type_of(request(&fixture, "switches.ts", &source, name, expand));
                    if !name.starts_with("inert") || name == "inertPairedDuplicate" {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{profile}/{name}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::UnsupportedSemantic,
                            "{error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::FlowRequired.into(),
                            "{error:?}"
                        );
                        assert_eq!(error.path, Some(fixture.path("switches.ts")));
                        assert_eq!(
                            serde_json::to_value(error.position.unwrap()).unwrap(),
                            row["location"]["start"]
                        );
                        assert!(error.causes.is_empty(), "{error:?}");
                        continue;
                    }
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{profile}/{name}: {answer:?}");
                    };
                    assert_eq!(
                        answer.outcome,
                        TypeOutcome::Found,
                        "{profile}/{name}: {answer:?}"
                    );
                    assert_eq!(
                        answer.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}"
                    );
                    assert_eq!(
                        serde_json::to_value(&answer.target.location).unwrap(),
                        row["location"]
                    );
                    assert!(answer.issues.is_empty(), "{answer:?}");
                }
            }
        }
    }
}

#[test]
fn enum_element_name_admission_is_located_at_the_demanded_argument() {
    let fixture = fixture();
    let source = std::fs::read_to_string(fixture.path("enum-keys.ts")).unwrap();
    for profile in ["strict", "loose"] {
        let rows: Vec<Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-enum-keys-{profile}.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 18);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("enum-keys-{profile}.json"));
            let mut rows: Vec<_> = rows.iter().collect();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                assert_eq!(row["errorType"], false);
                assert_eq!(row["codes"], json!([]));
                assert_eq!(row["demandFailures"], json!([]));
                for expand in [false, true, true, false] {
                    let answer =
                        project.type_of(request(&fixture, "enum-keys.ts", &source, name, expand));
                    if matches!(
                        name,
                        "keyPair" | "keyScalar" | "keyTemplatePair" | "keyTemplateScalar"
                    ) {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{profile}/{name}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{answer:?}");
                        assert_eq!(answer.description.summary, row["summary"].as_str().unwrap());
                        assert_eq!(
                            serde_json::to_value(&answer.target.location).unwrap(),
                            row["location"]
                        );
                        assert!(answer.issues.is_empty(), "{answer:?}");
                    } else {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{profile}/{name}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::UnsupportedSemantic,
                            "{error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::UnrepresentableIdentity.into(),
                            "{error:?}"
                        );
                        assert_eq!(error.path, Some(fixture.path("enum-keys.ts")));
                        let marker = format!("/*@key-{}*/", name.strip_prefix("key").unwrap());
                        assert_eq!(
                            error.position,
                            Some(position(
                                &source,
                                source.find(&marker).unwrap() + marker.len()
                            )),
                            "{error:?}"
                        );
                        assert!(error.causes.is_empty(), "{error:?}");
                    }
                }
            }
        }
    }
}

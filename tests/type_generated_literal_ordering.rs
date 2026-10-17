//! Final queries distinguish exact native literal byte ordering from the
//! separate generated property-key policy and unavailable decimal spellings.
mod support;

use serde_json::{Value, json};
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-generated-literal-ordering/cases.ts");
const CASES: &str = include_str!("../fixtures/types/native-generated-literal-ordering/cases.json");

fn fixture() -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-generated-literal-ordering"),
    )
}

fn request(fixture: &Fixture, name: &str, expand: bool) -> TypeRequest {
    let marker = format!("/*#{name}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len()),
        },
        expand,
        depth: 2,
    }
}

fn symbol(name: &str) -> Value {
    let function = SOURCE.find(&format!("function {name}<")).unwrap();
    let class = function + SOURCE[function..].find("class {").unwrap();
    json!({"kind":"symbol", "ref":"s0", "origins":[{
        "file":"cases.ts", "start":position(SOURCE, class), "end":position(SOURCE, class + 5),
    }]})
}

fn quoted(value: &str) -> String {
    if value == "\0" {
        "\"\\0\"".into()
    } else {
        serde_json::to_string(value).unwrap()
    }
}

// Closed public literal projection for the fixture's explicit $ID recipes.
// The native capture's concrete digits never become a product expectation.
fn literal(name: &str, index: usize, value: &str) -> (String, Value) {
    if name == "rawFE" && index == 1 {
        return (
            "\"\\uFFFD\"".into(),
            json!({"parts":[{"kind":"bytes", "value":"fe"}]}),
        );
    }
    if !value.contains("$ID") {
        return (quoted(value), json!(value));
    }
    let mut parts = Vec::new();
    let mut summaries = Vec::new();
    for (index, text) in value.split("$ID").enumerate() {
        if index != 0 {
            parts.push(symbol(name));
            summaries.push("symbol(s0)".to_owned());
        }
        if text.is_empty() {
            continue;
        }
        if name.starts_with("uniqueCycle")
            && let Some(tail) = text.strip_prefix('�')
        {
            let bytes = format!(
                "fe{}",
                tail.as_bytes()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            );
            summaries.push(format!("bytes(\"{bytes}\")"));
            parts.push(json!({"kind":"bytes", "value":bytes}));
        } else {
            summaries.push(quoted(text));
            parts.push(json!({"kind":"text", "value":text}));
        }
    }
    (
        format!("string({})", summaries.join(", ")),
        json!({"parts":parts}),
    )
}

#[test]
fn native_literal_order_is_exact_or_reports_its_decimal_dependency() {
    let fixture = fixture();
    let cases: Vec<Value> = serde_json::from_str(CASES).unwrap();
    for profile in ["strict", "loose"] {
        let native: Vec<Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(native.len(), 28);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut rows: Vec<_> = cases.iter().collect();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                let evidence = native.iter().find(|entry| entry["name"] == name).unwrap();
                assert_eq!(evidence["errorType"], false);
                assert_eq!(evidence["codes"], json!([]));
                assert_eq!(evidence["demandFailures"], json!([]));
                for expand in [false, true, true, false] {
                    let answer = project.type_of(request(&fixture, name, expand));
                    let Some(ordered) = row["ordered"].as_array() else {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::UnsupportedSemantic,
                            "{profile}/{name}/{expand}/{reverse}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::UnrepresentableIdentity.into(),
                            "{error:?}"
                        );
                        assert_eq!(
                            error.message,
                            "native string operation requires the opaque symbol identity's decimal spelling"
                        );
                        assert_eq!(error.path, Some(fixture.path("cases.ts")));
                        assert_eq!(
                            serde_json::to_value(error.position).unwrap(),
                            evidence["origins"][0]["location"]["start"],
                            "{error:?}"
                        );
                        continue;
                    };
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found, "{answer:?}");
                    assert!(answer.issues.is_empty(), "{answer:?}");
                    let values: Vec<_> = ordered
                        .iter()
                        .enumerate()
                        .map(|(index, value)| literal(name, index, value.as_str().unwrap()))
                        .collect();
                    let expected_summary = format!(
                        "[string(symbol(s0)), {}]",
                        values
                            .iter()
                            .map(|(summary, _)| summary.as_str())
                            .collect::<Vec<_>>()
                            .join(" | ")
                    );
                    assert_eq!(
                        answer.description.summary, expected_summary,
                        "{profile}/{name}/{expand}/{reverse}"
                    );
                    assert!(!answer.description.summary_truncated);
                    assert_eq!(answer.description.expansion.is_some(), expand);
                    if let Some(graph) = answer.description.expansion {
                        let graph = serde_json::to_value(graph).unwrap();
                        let nodes = graph["nodes"].as_array().unwrap();
                        let node = |reference: &Value| {
                            nodes
                                .iter()
                                .find(|node| node["id"] == reference["ref"])
                                .unwrap()
                        };
                        let root = nodes
                            .iter()
                            .find(|node| node["id"] == graph["root"])
                            .unwrap();
                        assert_eq!(root["kind"], "tuple");
                        assert!(root.get("blockedCollections").is_none(), "{root}");
                        let first = node(&root["elements"][0]["type"]);
                        assert_eq!(first["value"], json!({"parts":[symbol(name)]}));
                        let ordered_node = node(&root["elements"][1]["type"]);
                        let actual: Vec<_> = if values.len() == 1 {
                            vec![ordered_node]
                        } else {
                            assert_eq!(ordered_node["kind"], "union", "{ordered_node}");
                            ordered_node["alternatives"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(node)
                                .collect()
                        };
                        assert_eq!(actual.len(), values.len());
                        for (node, (summary, value)) in actual.iter().zip(&values) {
                            assert_eq!(node["kind"], "literal");
                            assert_eq!(node["base"], "string");
                            assert_eq!(node["value"], *value, "{profile}/{name}: {node}");
                            assert_eq!(node["summary"], *summary);
                            assert_eq!(node["summaryTruncated"], false);
                            assert!(node.get("omitted").is_none(), "{node}");
                        }
                    }
                }
            }
        }
    }
}

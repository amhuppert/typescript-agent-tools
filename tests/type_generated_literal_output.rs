//! Generated literal output keeps native bytes and presentation-local identity.
//! Native API/hook captures establish each semantic input; public wire bounds
//! and symbolic references follow the documented deterministic output contract.
mod support;

use serde_json::{Value, json};
use support::{Fixture, position};
use typescript_parser::analysis::{
    AnalysisError, SemanticErrorKind, SemanticFailureReason, SemanticResourceReason,
};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-generated-literal-output/cases.ts");
const HIDDEN: &str = include_str!("../fixtures/types/native-generated-literal-output/hidden.ts");
const BUDGET: &str = include_str!("../fixtures/types/native-generated-literal-output/budget.ts");
const GLOBALS: &str =
    include_str!("../fixtures/types/native-generated-literal-context/globals.d.ts");
const MAX_BYTES: usize = 262_144;

fn fixture() -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-generated-literal-output"),
    )
}

fn request(
    fixture: &Fixture,
    file: &str,
    source: &str,
    marker: &str,
    expand: bool,
    depth: usize,
) -> TypeRequest {
    let marker = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path(file),
            position: position(source, source.find(&marker).unwrap() + marker.len()),
        },
        expand,
        depth,
    }
}

fn class_origin(file: &str, source: &str, function: &str, ordinal: usize) -> Value {
    let start = source.find(&format!("function {function}<")).unwrap();
    let offset = start
        + source[start..]
            .match_indices("class {")
            .nth(ordinal)
            .unwrap()
            .0;
    json!({"file":file, "start":position(source, offset), "end":position(source, offset + 5)})
}

fn symbol(reference: &str, origin: &Value) -> Value {
    json!({"kind":"symbol", "ref":reference, "origins":[origin]})
}

fn text(value: &str) -> Value {
    json!({"kind":"text", "value":value})
}
fn parts(parts: Vec<Value>) -> Value {
    json!({"parts":parts})
}

#[test]
fn symbolic_output_preserves_bytes_shared_atoms_and_distinct_atoms() {
    let fixture = fixture();
    for mode in ["strict", "loose"] {
        let rows: Vec<Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{mode}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 7);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{mode}.json"));
            let mut ordered: Vec<_> = rows.iter().collect();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                assert_eq!(row["errorType"], false);
                assert_eq!(row["codes"], json!([]));
                let (summary, expected_values) = match name {
                    "rawPrefix" => (
                        row["summary"].as_str().unwrap().to_owned(),
                        vec![parts(vec![json!({"kind":"bytes", "value":"fe"})])],
                    ),
                    "repeated" => {
                        let atom = symbol("s0", &class_origin("cases.ts", SOURCE, name, 0));
                        (
                            "string(symbol(s0), \":\", symbol(s0))".to_owned(),
                            vec![parts(vec![atom.clone(), text(":"), atom])],
                        )
                    }
                    "sharedTuple" => {
                        let atom = symbol("s0", &class_origin("cases.ts", SOURCE, name, 0));
                        (
                            "[string(symbol(s0)), string(symbol(s0))]".to_owned(),
                            vec![parts(vec![atom])],
                        )
                    }
                    "sharedGraph" => {
                        let atom = symbol("s0", &class_origin("cases.ts", SOURCE, name, 0));
                        ("{ direct: string(symbol(s0)); prefixed: string(\"pre:\", symbol(s0)); again: string(symbol(s0)); }".to_owned(),
                            vec![parts(vec![atom.clone()]), parts(vec![text("pre:"), atom])])
                    }
                    "distinctTuple" => {
                        let outer = symbol("s0", &class_origin("cases.ts", SOURCE, name, 0));
                        let inner = symbol("s1", &class_origin("cases.ts", SOURCE, name, 1));
                        (
                            "[string(symbol(s0)), string(symbol(s1)), string(symbol(s0))]"
                                .to_owned(),
                            vec![parts(vec![outer]), parts(vec![inner])],
                        )
                    }
                    "replacement" => (
                        row["summary"].as_str().unwrap().to_owned(),
                        vec![json!("�")],
                    ),
                    "lookalike" => (
                        row["summary"].as_str().unwrap().to_owned(),
                        vec![json!("string(symbol(s0))")],
                    ),
                    _ => panic!("unknown native output row {name}"),
                };
                for expand in [false, true, true, false] {
                    let answer =
                        project.type_of(request(&fixture, "cases.ts", SOURCE, name, expand, 2));
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{mode}/{name}/{expand}/{reverse}: {answer:?}");
                    };
                    if name == "distinctTuple" && expand {
                        assert_eq!(answer.outcome, TypeOutcome::Partial, "{answer:?}");
                        assert_eq!(answer.issues.len(), 1, "{answer:?}");
                        let issue = &answer.issues[0];
                        assert_eq!(issue.reason, SemanticFailureReason::UnrepresentableIdentity);
                        assert_eq!(
                            issue.message,
                            "native string operation requires the opaque symbol identity's decimal spelling"
                        );
                        let tuple = "[typeof outer, IdentityPart<Q>, typeof outer]";
                        let start = SOURCE.find(tuple).unwrap();
                        assert_eq!(issue.location.file, "cases.ts");
                        assert_eq!(issue.location.start, position(SOURCE, start));
                        assert_eq!(issue.location.end, position(SOURCE, start + tuple.len()));
                    } else {
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{answer:?}");
                        assert!(answer.issues.is_empty(), "{answer:?}");
                    }
                    assert_eq!(
                        answer.description.summary, summary,
                        "{mode}/{name}/{expand}/{reverse}"
                    );
                    assert!(!answer.description.summary_truncated);
                    if let Some(graph) = answer.description.expansion {
                        // Tuple members can reach inherited Array signatures
                        // beyond depth two; their literal elements remain exact.
                        if !matches!(name, "sharedTuple" | "distinctTuple") {
                            assert!(!graph.truncated, "{mode}/{name}: {graph:?}");
                        }
                        let graph = serde_json::to_value(graph).unwrap();
                        let nodes = graph["nodes"].as_array().unwrap();
                        let literals: Vec<_> = nodes
                            .iter()
                            .filter(|node| node["kind"] == "literal" && node["base"] == "string")
                            .collect();
                        assert_eq!(
                            literals
                                .iter()
                                .map(|node| node["value"].clone())
                                .collect::<Vec<_>>(),
                            expected_values,
                            "{mode}/{name}"
                        );
                        for node in &literals {
                            assert!(node.get("omitted").is_none(), "{node}");
                            assert_eq!(node["summaryTruncated"], false, "{node}");
                            if let Some(parts) = node["value"]["parts"].as_array() {
                                for part in parts.iter().filter(|part| part["kind"] == "symbol") {
                                    assert!(
                                        node["summary"].as_str().unwrap().contains(&format!(
                                            "symbol({})",
                                            part["ref"].as_str().unwrap()
                                        )),
                                        "{node}"
                                    );
                                }
                            }
                        }
                        let root = &nodes[0];
                        if name == "sharedTuple" {
                            assert_eq!(root["elements"][0]["type"], root["elements"][1]["type"]);
                        } else if name == "distinctTuple" {
                            let issue = serde_json::to_value(&answer.issues[0]).unwrap();
                            assert_eq!(
                                root["blockedCollections"],
                                json!([
                                    {"field":"properties", "issue":issue},
                                    {"field":"callSignatures", "issue":issue},
                                    {"field":"constructSignatures", "issue":issue},
                                    {"field":"indexSignatures", "issue":issue},
                                ])
                            );
                            assert_eq!(root["elements"][0]["type"], root["elements"][2]["type"]);
                            assert_ne!(root["elements"][0]["type"], root["elements"][1]["type"]);
                        } else if name == "sharedGraph" {
                            let properties = root["properties"].as_array().unwrap();
                            let property = |name: &str| {
                                properties
                                    .iter()
                                    .find(|row| row["key"]["name"] == name)
                                    .unwrap()
                            };
                            assert_eq!(property("direct")["type"], property("again")["type"]);
                            assert_ne!(property("direct")["type"], property("prefixed")["type"]);
                        }
                    }
                }
                if name == "distinctTuple" {
                    let answer =
                        project.type_of(request(&fixture, "cases.ts", SOURCE, name, true, 0));
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{answer:?}");
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found, "{answer:?}");
                    assert!(answer.issues.is_empty());
                    assert_eq!(answer.description.summary, summary);
                }
            }
        }
    }
}

#[test]
fn elided_summary_atoms_do_not_claim_visible_references() {
    let fixture = fixture();
    for mode in ["strict", "loose"] {
        let rows: Vec<Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-hidden-{mode}.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["errorType"], false);
        let project = fixture.load_config(&format!("hidden-{mode}.json"));
        for (expand, depth) in [(false, 0), (true, 0), (true, 2), (false, 2)] {
            let answer = project.type_of(request(
                &fixture,
                "hidden.ts",
                HIDDEN,
                "hiddenSummary",
                expand,
                depth,
            ));
            let Ok(TypeAnswer::Selected(answer)) = answer else {
                panic!("{mode}/{expand}/{depth}: {answer:?}");
            };
            assert_eq!(answer.outcome, TypeOutcome::Found, "{answer:?}");
            assert!(answer.issues.is_empty());
            assert_eq!(answer.description.summary, "[…, string(symbol(s0))]");
            assert!(answer.description.summary_truncated);
            if let Some(graph) = answer.description.expansion {
                assert!(graph.truncated);
                assert_eq!(graph.nodes[0].summary, answer.description.summary);
                if depth == 2 {
                    let nodes = serde_json::to_value(&graph.nodes).unwrap();
                    let values: Vec<_> = nodes
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|node| node["kind"] == "literal" && node["base"] == "string")
                        .map(|node| node["value"].clone())
                        .collect();
                    let outer =
                        symbol("s1", &class_origin("hidden.ts", HIDDEN, "hiddenSummary", 0));
                    let inner =
                        symbol("s0", &class_origin("hidden.ts", HIDDEN, "hiddenSummary", 1));
                    assert_eq!(
                        values,
                        vec![
                            parts(vec![text(&"x".repeat(600)), outer]),
                            parts(vec![inner])
                        ]
                    );
                }
            }
        }
    }
}

// Independent expected wire shape. The generated source deliberately keeps
// the anonymous-class origin on a fixed line, outside the variable text.
fn budget_wire(padding: usize) -> Value {
    json!({"summary":"…", "summaryTruncated":true, "expansion":{"root":"t0", "nodes":[{
        "id":"t0", "summary":"…", "summaryTruncated":true, "kind":"literal", "base":"string",
        "value":{"parts":[text(&"x".repeat(padding)), symbol("s0", &json!({"file":"budget.ts",
            "start":{"line":5,"column":5}, "end":{"line":5,"column":10}}))]}
    }], "truncated":true}})
}

#[test]
fn exact_symbolic_root_metadata_obeys_the_display_byte_boundary() {
    let baselines = fixture();
    let overhead = serde_json::to_vec(&budget_wire(0)).unwrap().len();
    for mode in ["strict", "loose"] {
        let native: Vec<Value> = serde_json::from_str(
            &std::fs::read_to_string(baselines.path(&format!("expected-budget-{mode}.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(native.len(), 3);
        for (row, bytes) in native.iter().zip([MAX_BYTES - 1, MAX_BYTES, MAX_BYTES + 1]) {
            let padding = bytes - overhead;
            let expected = budget_wire(padding);
            assert_eq!(serde_json::to_vec(&expected).unwrap().len(), bytes);
            assert_eq!(row["bytes"], bytes);
            assert_eq!(row["padding"], padding);
            assert_eq!(row["errorType"], false);
            assert_eq!(row["codes"], json!([]));
            let source = BUDGET.replace("__PADDING__", &"x".repeat(padding));
            let config = format!(
                r#"{{"compilerOptions":{{"strict":{},"target":"ESNext","module":"ESNext","noLib":true,"types":[]}},"files":["globals.d.ts","budget.ts"]}}"#,
                mode == "strict"
            );
            let fixture = Fixture::new(&[
                ("tsconfig.json", &config),
                ("globals.d.ts", GLOBALS),
                ("budget.ts", &source),
            ]);
            let project = fixture.load_config("tsconfig.json");
            for expand in [false, true] {
                let answer =
                    project.type_of(request(&fixture, "budget.ts", &source, "budget", expand, 0));
                if expand && bytes > MAX_BYTES {
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{mode}/{bytes}: {answer:?}");
                    };
                    assert_eq!(error.kind, SemanticErrorKind::OutputLimit);
                    assert_eq!(error.reason, SemanticResourceReason::OutputLimit.into());
                } else {
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{mode}/{bytes}/{expand}: {answer:?}");
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found, "{answer:?}");
                    assert!(answer.issues.is_empty());
                    assert_eq!(answer.description.summary, "…");
                    assert!(answer.description.summary_truncated);
                    if expand {
                        assert_eq!(serde_json::to_value(answer.description).unwrap(), expected);
                    }
                }
            }
        }
    }
}

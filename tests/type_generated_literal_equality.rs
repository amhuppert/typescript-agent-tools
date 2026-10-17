//! Semantic equality observes whole opaque atoms without guessing their digits.
//! Native tuples record actual IDs only as evidence; product references and
//! origins follow the exact symbolic-output contract.
mod support;

use serde_json::{Value, json};
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-generated-literal-equality/cases.ts");

fn fixture() -> Fixture {
    Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-generated-literal-equality"),
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

fn symbol_value(name: &str, ordinal: usize) -> Value {
    let function = SOURCE.find(&format!("function {name}<")).unwrap();
    let class = function
        + SOURCE[function..]
            .match_indices("class {")
            .nth(ordinal)
            .unwrap()
            .0;
    json!({"parts":[{"kind":"symbol", "ref":format!("s{ordinal}"), "origins":[{
        "file":"cases.ts", "start":position(SOURCE, class), "end":position(SOURCE, class + 5),
    }]}]})
}

#[test]
fn whole_atom_and_fixed_suffix_equality_keep_ambiguous_partitions_explicit() {
    let fixture = fixture();
    for profile in ["strict", "loose"] {
        let rows: Vec<Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 3);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered: Vec<_> = rows.iter().collect();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                assert!(matches!(
                    name,
                    "wholeDistinct" | "adjacentPartition" | "suffixDecides"
                ));
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert_eq!(row["codes"], json!([]), "{profile}/{name}");
                assert_eq!(row["demandFailures"], json!([]), "{profile}/{name}");
                assert_eq!(row["identities"].as_array().unwrap().len(), 2);
                assert_ne!(row["identities"][0], row["identities"][1]);
                assert_eq!(row["comparison"], false, "{profile}/{name}");
                for expand in [false, true, true, false] {
                    let answer = project.type_of(request(&fixture, name, expand));
                    if name == "adjacentPartition" {
                        // Different atom partitions can spell identical bytes:
                        // IDs 1 and 11 would make both concatenations "111".
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
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
                        assert_eq!(error.path, Some(fixture.path("cases.ts")), "{error:?}");
                        let declaration = SOURCE
                            .find("static #inner = (inner) => /*#adjacentPartition*/inner;")
                            .unwrap();
                        let parameter = declaration + "static #inner = (".len();
                        assert_eq!(
                            error.position,
                            Some(position(SOURCE, parameter)),
                            "{error:?}"
                        );
                        continue;
                    }
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
                    };
                    if expand {
                        assert_eq!(answer.outcome, TypeOutcome::Partial, "{answer:?}");
                        assert_eq!(answer.issues.len(), 1, "{answer:?}");
                        let issue = &answer.issues[0];
                        assert_eq!(issue.reason, SemanticFailureReason::UnrepresentableIdentity);
                        assert_eq!(
                            issue.message,
                            "native string operation requires the opaque symbol identity's decimal spelling"
                        );
                        let function = SOURCE.find(&format!("function {name}<")).unwrap();
                        let start = function + SOURCE[function..].find("[typeof outer,").unwrap();
                        let end = start + SOURCE[start..].find("]) =>").unwrap() + 1;
                        assert_eq!(issue.location.file, "cases.ts");
                        assert_eq!(issue.location.start, position(SOURCE, start));
                        assert_eq!(issue.location.end, position(SOURCE, end));
                    } else {
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{answer:?}");
                        assert!(
                            answer.issues.is_empty(),
                            "{profile}/{name}: {:?}",
                            answer.issues
                        );
                    }
                    assert_eq!(
                        answer.description.summary,
                        "[string(symbol(s0)), string(symbol(s1)), false]",
                        "{profile}/{name}/{expand}/{reverse}"
                    );
                    assert!(!answer.description.summary_truncated);
                    assert_eq!(answer.description.expansion.is_some(), expand);
                    if let Some(graph) = answer.description.expansion {
                        let graph = serde_json::to_value(graph).unwrap();
                        let nodes = graph["nodes"].as_array().unwrap();
                        let root = nodes
                            .iter()
                            .find(|node| node["id"] == graph["root"])
                            .unwrap();
                        assert_eq!(root["kind"], "tuple");
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
                        assert_eq!(root["summary"], answer.description.summary);
                        let elements = root["elements"].as_array().unwrap();
                        assert_eq!(elements.len(), 3);
                        assert_ne!(elements[0]["type"], elements[1]["type"]);
                        for (index, element) in elements.iter().enumerate() {
                            let reference = element["type"]["ref"].as_str().unwrap();
                            let node = nodes.iter().find(|node| node["id"] == reference).unwrap();
                            assert_eq!(node["kind"], "literal");
                            assert_eq!(node["summaryTruncated"], false);
                            assert!(node.get("omitted").is_none(), "{node}");
                            if index < 2 {
                                assert_eq!(node["base"], "string");
                                assert_eq!(
                                    node["value"],
                                    symbol_value(name, index),
                                    "{profile}/{name}"
                                );
                                assert_eq!(node["summary"], format!("string(symbol(s{index}))"));
                            } else {
                                assert_eq!(node["base"], "boolean");
                                assert_eq!(node["value"], "false");
                                assert_eq!(node["summary"], "false");
                            }
                        }
                    }
                }
                if name != "adjacentPartition" {
                    let mut selected = request(&fixture, name, true);
                    selected.depth = 0;
                    let answer = project.type_of(selected);
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{answer:?}");
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found, "{answer:?}");
                    assert!(answer.issues.is_empty());
                    assert_eq!(
                        answer.description.summary,
                        "[string(symbol(s0)), string(symbol(s1)), false]"
                    );
                }
            }
        }
    }
}

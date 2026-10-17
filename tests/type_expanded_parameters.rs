//! Native rest-tuple expansion supplies presentation parameters. The graph
//! retains the original signature inventory and canonical parameter identities.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-expanded-parameters/cases.ts");

#[test]
fn rest_tuple_display_expands_without_replacing_signature_parameters() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-expanded-parameters/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-expanded-parameters/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-expanded-parameters/loose.json"),
        ),
    ]);
    for (profile, expected) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-expanded-parameters/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-expanded-parameters/expected-loose.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("{profile}.json"));
        let rows: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        assert_eq!(rows.len(), 16);
        // Reversed queries check request isolation, not an evaluator warm cache.
        for reverse in [false, true] {
            let mut ordered = rows.iter().collect::<Vec<_>>();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                let marker = format!("/*#{name}*/");
                let mut matches = SOURCE.match_indices(&marker);
                let (offset, _) = matches.next().unwrap();
                assert!(matches.next().is_none(), "ambiguous {marker}");
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert_eq!(row["before"], serde_json::json!([]), "{profile}/{name}");
                assert_eq!(row["after"], serde_json::json!([]), "{profile}/{name}");
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, offset + marker.len()),
                        },
                        expand,
                        depth: if expand { 1 } else { 0 },
                    });
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{profile}/{name}/{expand}/{reverse}: {answer:?}");
                    };
                    assert_eq!(
                        result.outcome,
                        TypeOutcome::Found,
                        "{profile}/{name}: {result:?}"
                    );
                    assert_eq!(
                        result.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}/{expand}/{reverse}"
                    );
                    assert!(!result.description.summary_truncated, "{profile}/{name}");
                    assert!(result.issues.is_empty(), "{profile}/{name}: {result:?}");
                    if expand {
                        let graph = result.description.expansion.as_ref().unwrap();
                        let root = graph
                            .nodes
                            .iter()
                            .find(|node| node.id == graph.root)
                            .unwrap();
                        let TypeNodeData::Function { members } = &root.data else {
                            panic!("{profile}/{name}: {root:?}");
                        };
                        let signatures = members.call_signatures.as_ref().unwrap();
                        assert_eq!(signatures.len(), 1, "{profile}/{name}");
                        let parameters = &signatures[0].parameters;
                        let prefix = name == "prefix";
                        assert_eq!(
                            parameters.len(),
                            if prefix { 2 } else { 1 },
                            "{profile}/{name}"
                        );
                        let rest = parameters.last().unwrap();
                        let symbol_name = if matches!(name, "binding" | "omitted" | "nested") {
                            "__0"
                        } else {
                            "args"
                        };
                        assert_eq!(rest.name, symbol_name, "{profile}/{name}");
                        if prefix {
                            assert_eq!(parameters[0].name, "before");
                        }
                    }
                }
            }
        }
    }
}

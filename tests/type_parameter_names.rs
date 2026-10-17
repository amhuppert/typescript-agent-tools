//! Declaration syntax supplies displayed names; canonical parameter symbols
//! remain distinct metadata in the expanded graph.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-parameter-names/cases.ts");

#[test]
fn binding_parameter_names_preserve_native_syntax_and_symbol_identity() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-parameter-names/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-parameter-names/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-parameter-names/loose.json"),
        ),
    ]);
    for (profile, expected) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-parameter-names/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-parameter-names/expected-loose.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("{profile}.json"));
        let rows: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        assert_eq!(rows.len(), 23);
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
                        assert_eq!(signatures[0].parameters.len(), 1, "{profile}/{name}");
                        // Native binder names anonymous binding parameters by
                        // their parameter-list index, independently of printing.
                        let symbol_name = if matches!(name, "identifier" | "escapedIdentifier") {
                            "value"
                        } else {
                            "__0"
                        };
                        assert_eq!(
                            signatures[0].parameters[0].name, symbol_name,
                            "{profile}/{name}"
                        );
                    }
                }
            }
        }
    }
}

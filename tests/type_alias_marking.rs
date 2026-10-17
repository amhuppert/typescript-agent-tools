//! getResolvedSymbol applies its lookup mask to the retained lexical scope,
//! including identifiers navigation classified as import-equals namespaces.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-alias-marking/cases.ts");

#[test]
fn import_equals_marking_resolves_namespace_heads_as_actual_value_reads() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-alias-marking"),
    );
    for (profile, baseline) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-alias-marking/expected-tsconfig.json"),
        ),
        (
            "verbatim",
            include_str!("../fixtures/types/native-alias-marking/expected-verbatim.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 3);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut rows = rows.clone();
            if reverse {
                rows.reverse();
            }
            for row in rows {
                let name = row["name"].as_str().unwrap();
                assert_eq!(row["summary"], "number");
                assert_eq!(row["errorType"], false);
                let marker = format!("/*#{name}*/");
                let offset = SOURCE.find(&marker).unwrap() + marker.len();
                for expand in [false, true] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let result = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, offset),
                        },
                        expand,
                        depth: 1,
                    });
                    let Ok(TypeAnswer::Selected(answer)) = result else {
                        panic!("{label}: {result:?}");
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found, "{label}: {answer:?}");
                    assert_eq!(answer.description.summary, "number", "{label}");
                    assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                    if let Some(graph) = answer.description.expansion {
                        assert!(
                            graph
                                .nodes
                                .iter()
                                .all(|node| matches!(node.data, TypeNodeData::Primitive { .. })),
                            "{label}: {graph:?}"
                        );
                    }
                }
            }
        }
    }
}

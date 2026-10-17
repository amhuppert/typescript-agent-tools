//! Native parameter-property inventory includes malformed binding patterns;
//! only duplicate-member checking excludes them. Deferred member annotations
//! remain untouched when override diagnostics merely display the class name.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-class-heritage-variants/cases.ts");

#[test]
fn class_parameter_property_patterns_preserve_native_query_dependencies() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-class-heritage-variants"),
    );
    for (profile, baseline) in [
        (
            "strict",
            include_str!("../fixtures/types/native-class-heritage-variants/expected-strict.json"),
        ),
        (
            "nocheck",
            include_str!("../fixtures/types/native-class-heritage-variants/expected-nocheck.json"),
        ),
        (
            "override",
            include_str!("../fixtures/types/native-class-heritage-variants/expected-override.json"),
        ),
    ] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(baseline).unwrap();
        assert_eq!(rows.len(), 8);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered = rows.clone();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let name = row["name"].as_str().unwrap();
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert!(row["demandFailures"].as_array().unwrap().is_empty());
                assert!(
                    !row["acquisitions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|event| event["event"] == "exit" && event["errorType"] == true),
                    "{profile}/{name}"
                );
                let has_override = name.ends_with("Pattern");
                assert_eq!(
                    row["codes"],
                    if has_override {
                        serde_json::json!([4112])
                    } else {
                        serde_json::json!([])
                    },
                    "{profile}/{name}"
                );
                let marker = format!("/*#{name}*/");
                let offset = SOURCE.find(&marker).unwrap() + marker.len();
                for expand in [false, true] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, offset),
                        },
                        expand,
                        depth: 1,
                    });
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
                    if let Some(graph) = answer.description.expansion {
                        assert!(
                            graph
                                .nodes
                                .iter()
                                .all(|node| !matches!(node.data, TypeNodeData::Unsupported { .. })),
                            "{label}: {graph:?}"
                        );
                    }
                }
            }
        }
    }
}

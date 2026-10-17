//! Generated private names preserve class identity without process numeric IDs.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{
    TypeAnswer, TypeExpansion, TypeKey, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/native-private-names/cases.ts");

fn summary<'a>(graph: &'a TypeExpansion, reference: &'a TypeRef) -> &'a str {
    match reference {
        TypeRef::Ref { r#ref } => {
            &graph
                .nodes
                .iter()
                .find(|node| node.id == *r#ref)
                .unwrap()
                .summary
        }
        TypeRef::Omitted { summary, .. } => summary,
    }
}

#[test]
fn private_names_keep_inherited_owners_accessors_and_literal_name_collisions_distinct() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-private-names"),
    );
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/types/native-private-names/expected.json"
    ))
    .unwrap();
    for reverse in [false, true] {
        let project = fixture.load();
        let mut rows: Vec<_> = expected["rows"].as_array().unwrap().iter().collect();
        if reverse {
            rows.reverse();
        }
        for row in rows {
            let name = row["name"].as_str().unwrap();
            let marker = format!("/*#{name}*/");
            let offset = SOURCE.find(&marker).unwrap() + marker.len();
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, offset),
                });
                request.expand = expand;
                request.depth = 2;
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(answer)) = answer else {
                    panic!("{name}/{expand}/{reverse}: {answer:?}");
                };
                assert_eq!(answer.outcome, TypeOutcome::Found, "{name}: {answer:?}");
                assert_eq!(
                    answer.description.summary,
                    row["summary"].as_str().unwrap(),
                    "{name}"
                );
                assert!(answer.issues.is_empty(), "{name}: {answer:?}");
                if !expand {
                    continue;
                }
                // This marker selects a private field, not its anonymous class.
                if name == "anonymousMember" {
                    continue;
                }
                let inventory = expected["inventories"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|inventory| inventory["name"] == row["summary"])
                    .unwrap_or_else(|| {
                        panic!("missing native inventory for {name}: {}", row["summary"])
                    });
                let graph = answer.description.expansion.unwrap();
                let root = graph
                    .nodes
                    .iter()
                    .find(|node| node.id == graph.root)
                    .unwrap();
                let members = match (&root.data, name) {
                    (TypeNodeData::Function { members }, "staticSide") => members,
                    (TypeNodeData::Object { members }, name) if name != "staticSide" => members,
                    _ => panic!("{name}: {root:?}"),
                };
                let mut properties: Vec<_> = members.properties.as_ref().unwrap().iter().map(|property| {
                    let (kind, name) = match &property.key {
                        TypeKey::String { name } => ("string", name),
                        TypeKey::Private { name, origins } => {
                            assert_eq!(origins, &property.origins);
                            ("private", name)
                        }
                        _ => panic!("unexpected private fixture key"),
                    };
                    serde_json::json!({"kind":kind,"name":name,"summary":summary(&graph,&property.read_type),
                        "origins":property.origins.iter().map(|origin| {
                            assert_eq!(origin.file,"cases.ts");
                            serde_json::json!({"line":origin.start.line,"column":origin.start.column,"end":origin.end})
                        }).collect::<Vec<_>>()})
                }).collect();
                properties.sort_by(|a, b| {
                    (
                        a["kind"].as_str(),
                        a["name"].as_str(),
                        a["origins"][0]["line"].as_u64(),
                        a["origins"][0]["column"].as_u64(),
                    )
                        .cmp(&(
                            b["kind"].as_str(),
                            b["name"].as_str(),
                            b["origins"][0]["line"].as_u64(),
                            b["origins"][0]["column"].as_u64(),
                        ))
                });
                let expected_properties: Vec<_> = inventory["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|property| {
                        let mut expected = property.clone();
                        let origin = expected
                            .as_object_mut()
                            .unwrap()
                            .remove("graphOrigin")
                            .unwrap();
                        if !origin.is_null() {
                            expected["summary"] = format!(
                                "{} ({}:{}:{})",
                                property["summary"].as_str().unwrap(),
                                origin["file"].as_str().unwrap(),
                                origin["line"].as_u64().unwrap(),
                                origin["column"].as_u64().unwrap()
                            )
                            .into();
                        }
                        expected
                    })
                    .collect();
                assert_eq!(properties, expected_properties, "{name}/{reverse}");
            }
        }
    }
}

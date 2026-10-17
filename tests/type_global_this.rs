//! One initialized native symbol participates in lookup and compatible merges.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{
    TypeAnswer, TypeExpansion, TypeKey, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

fn request(
    fixture: &Fixture,
    scenario: &str,
    name: &str,
    expand: bool,
    depth: usize,
) -> TypeRequest {
    let file = format!("{scenario}.ts");
    let source = std::fs::read_to_string(fixture.path(&file)).unwrap();
    let marker = format!("/*#{name}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path(&file),
        position: position(&source, source.find(&marker).unwrap() + marker.len()),
    });
    request.expand = expand;
    request.depth = depth;
    request
}

#[test]
fn initialized_global_this_identity_survives_merges_and_respects_lexical_shadowing() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/native-global-this"),
    );
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-global-this/expected.json"
    ))
    .unwrap();
    for row in rows {
        let scenario = row["scenario"].as_str().unwrap();
        let name = row["name"].as_str().unwrap();
        let project = fixture.load_config(&format!("{scenario}.json"));
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, scenario, name, expand, 0));
            let Ok(TypeAnswer::Selected(answer)) = answer else {
                panic!("{scenario}/{name}/{expand}: {answer:?}");
            };
            assert_eq!(
                answer.outcome,
                TypeOutcome::Found,
                "{scenario}/{name}/{expand}: {answer:?}"
            );
            assert_eq!(
                answer.description.summary,
                row["summary"].as_str().unwrap(),
                "{scenario}/{name}/{expand}"
            );
            assert!(
                answer.issues.is_empty(),
                "{scenario}/{name}/{expand}: {answer:?}"
            );
        }
    }
}

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

// Native spelling plus the design's stable nominal declaration qualifier.
fn graph_label(native: &serde_json::Value) -> String {
    let summary = native["summary"].as_str().unwrap();
    let origin = &native["graphOrigin"];
    if origin.is_null() {
        return summary.to_owned();
    }
    format!(
        "{} ({}:{}:{})",
        summary,
        origin["file"].as_str().unwrap(),
        origin["line"].as_u64().unwrap(),
        origin["column"].as_u64().unwrap()
    )
}

#[test]
fn global_this_members_keep_global_filter_and_merged_signatures() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/native-global-this"),
    );
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-global-this/expected-inventory.json"
    ))
    .unwrap();
    for row in rows {
        let scenario = row["scenario"].as_str().unwrap();
        let project = fixture.load_config(&format!("{scenario}.json"));
        let answer = project.type_of(request(&fixture, scenario, "this", true, 2));
        let Ok(TypeAnswer::Selected(answer)) = answer else {
            panic!("{scenario}: {answer:?}");
        };
        assert_eq!(answer.outcome, TypeOutcome::Found, "{scenario}: {answer:?}");
        assert!(answer.issues.is_empty(), "{scenario}: {answer:?}");
        let graph = answer.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        let (TypeNodeData::Object { members } | TypeNodeData::Function { members }) = &root.data
        else {
            panic!("{scenario}: {root:?}");
        };
        let mut actual: Vec<_> = members
            .properties
            .as_ref()
            .unwrap()
            .iter()
            .map(|property| {
                let TypeKey::String { name } = &property.key else {
                    panic!("{scenario}: unexpected key");
                };
                (
                    name.as_str(),
                    summary(&graph, &property.read_type).to_owned(),
                )
            })
            .collect();
        actual.sort_unstable();
        let expected: Vec<_> = row["properties"]
            .as_array()
            .unwrap()
            .iter()
            .map(|property| (property["name"].as_str().unwrap(), graph_label(property)))
            .collect();
        assert_eq!(actual, expected, "{scenario}");
        for (actual, expected) in [
            (
                members.call_signatures.as_ref().unwrap(),
                row["calls"].as_array().unwrap(),
            ),
            (
                members.construct_signatures.as_ref().unwrap(),
                row["constructors"].as_array().unwrap(),
            ),
        ] {
            assert_eq!(actual.len(), expected.len(), "{scenario}");
            for (signature, expected) in actual.iter().zip(expected) {
                assert_eq!(
                    signature.parameters.len(),
                    expected["parameters"].as_array().unwrap().len(),
                    "{scenario}"
                );
                assert_eq!(
                    summary(&graph, &signature.return_type),
                    graph_label(&expected["returns"]),
                    "{scenario}"
                );
            }
        }
        let indexes: Vec<_> = members
            .index_signatures
            .as_ref()
            .unwrap()
            .iter()
            .map(|index| {
                (
                    summary(&graph, &index.key_type),
                    summary(&graph, &index.value_type),
                )
            })
            .collect();
        let expected: Vec<_> = row["indexes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|index| {
                (
                    index["key"].as_str().unwrap(),
                    index["value"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(indexes, expected, "{scenario}");
    }
}

//! Equal key sets must retain the native origin chosen by each actual query.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/native-keyof-origin-identity/cases.ts");

fn check_origin(marker: &str, expected: &str) {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-keyof-origin-identity"),
    );
    for profile in ["strict", "loose", "nocheck"] {
        let project = fixture.load_config(&format!("{profile}.json"));
        for expand in [false, true] {
            let request = TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(
                        SOURCE,
                        SOURCE.find(&format!("const {marker}Chosen")).unwrap() + 6,
                    ),
                },
                expand,
                depth: 2,
            };
            let TypeAnswer::Selected(result) = project.type_of(request).unwrap() else {
                panic!("expected selected {profile}/{marker}");
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(
                result.issues.is_empty(),
                "{profile}/{marker}: {:?}",
                result.issues
            );
            assert_eq!(
                result.description.summary, expected,
                "{profile}/{marker}/{expand}"
            );
            if let Some(graph) = &result.description.expansion {
                let root = graph
                    .nodes
                    .iter()
                    .find(|node| node.id == graph.root)
                    .unwrap();
                let TypeNodeData::Union { alternatives, .. } = &root.data else {
                    panic!("{profile}/{marker}: {:?}", root.data);
                };
                let summaries: Vec<_> = alternatives
                    .iter()
                    .map(|reference| match reference {
                        TypeRef::Ref { r#ref } => graph
                            .nodes
                            .iter()
                            .find(|node| &node.id == r#ref)
                            .unwrap()
                            .summary
                            .as_str(),
                        TypeRef::Omitted { .. } => panic!("small key union must be fully expanded"),
                    })
                    .collect();
                assert_eq!(summaries, ["\"first\"", "\"second\""]);
            }
        }
    }
}

#[test]
fn interface_key_origins_keep_the_selected_target() {
    check_origin("interface", "keyof InterfaceB");
}

#[test]
fn reverse_interface_acquisition_keeps_the_selected_target() {
    check_origin("reverse", "keyof InterfaceA");
}

#[test]
fn class_key_origins_keep_the_selected_target() {
    check_origin("class", "keyof ClassB");
}

#[test]
fn alias_key_origins_keep_the_selected_target() {
    check_origin("alias", "keyof AliasB");
}

#[test]
fn a_plain_union_cannot_absorb_a_later_keyof_origin() {
    check_origin("plainFirst", "keyof InterfaceB");
}

#[test]
fn a_keyof_origin_cannot_absorb_a_later_plain_union() {
    check_origin("indexFirst", "\"first\" | \"second\"");
}

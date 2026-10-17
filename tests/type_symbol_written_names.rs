//! Symbol name serialization and type-parameter recipes ask different questions.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/symbol-written-names/input.ts");

#[test]
fn type_parameter_display_uses_all_own_names_while_identity_and_lazy_recipes_stay_separate() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/symbol-written-names/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/symbol-written-names/loose.json"),
        ),
        ("input.ts", SOURCE),
    ]);
    let cases = [
        ("bare", "T"),
        ("quoted", "\"T\""),
        ("quotedEscape", "\"\\u0054\""),
        ("identifierEscape", "\\u0054"),
        ("computed", "[\"T\"]"),
    ];
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (name, expected) in cases {
            let marker = format!("/*#{name}*/");
            let mut request = TypeRequest::new(TypeQuery::At {
                file: fixture.path("input.ts"),
                position: position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len()),
            });
            request.expand = true;
            request.depth = 3;
            let answer = project.type_of(request).unwrap();
            let TypeAnswer::Selected(result) = answer else {
                panic!("own parameter is selected")
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, expected, "{config} {name}");
            assert!(result.issues.is_empty());
            let graph = result.description.expansion.unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            let TypeNodeData::TypeParameter {
                name,
                constraint,
                default,
                ..
            } = &root.data
            else {
                panic!("actual canonical parameter has its own declared type")
            };
            assert_eq!(name, "T");
            assert!(constraint.is_none());
            let Some(TypeRef::Ref { r#ref }) = default else {
                panic!("the independent first nonnil parameter default is established")
            };
            assert!(matches!(
                graph
                    .nodes
                    .iter()
                    .find(|node| node.id == *r#ref)
                    .unwrap()
                    .data,
                TypeNodeData::Unknown
            ));
        }
    }
}

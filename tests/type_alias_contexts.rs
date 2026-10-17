//! Anonymous alias contexts retain actual native declaration roles.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/native-generic-aliases/contexts.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("contexts.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-generic-aliases/contexts.json"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str, depth: usize) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("contexts.ts"),
        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
    });
    request.expand = true;
    request.depth = depth;
    request
}

#[test]
fn anonymous_method_uses_first_and_all_native_symbol_declarations() {
    let fixture = fixture();
    let project = fixture.load();
    let answer = project.type_of(request(&fixture, "methods", 2));
    let Ok(TypeAnswer::Selected(result)) = answer else {
        panic!("{answer:?}")
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert!(result.issues.is_empty(), "{:?}", result.issues);
    let graph = result.description.expansion.unwrap();
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    let TypeNodeData::Union { alternatives, .. } = &root.data else {
        panic!("{root:?}")
    };
    assert_eq!(alternatives.len(), 2);
    let methods: Vec<_> = alternatives
        .iter()
        .map(|alternative| {
            let TypeRef::Ref { r#ref } = alternative else {
                panic!("{alternative:?}")
            };
            let node = graph.nodes.iter().find(|node| node.id == *r#ref).unwrap();
            let TypeNodeData::Object { members } = &node.data else {
                panic!("{node:?}")
            };
            let properties = members.properties.as_ref().unwrap();
            assert_eq!(properties.len(), 1);
            let TypeRef::Ref { r#ref } = &properties[0].read_type else {
                panic!("{:?}", properties[0])
            };
            let method = graph.nodes.iter().find(|node| node.id == *r#ref).unwrap();
            assert_eq!(method.summary, "() => number");
            method.id.clone()
        })
        .collect();
    assert_ne!(methods[0], methods[1]);
}

#[test]
fn unnamed_function_and_constructor_types_retain_enclosing_alias_parameters() {
    let fixture = fixture();
    let project = fixture.load();
    for (marker, summary) in [
        ("callable", "Callable<string>"),
        ("constructor", "Constructor<number>"),
    ] {
        for depth in [0, 1, 0] {
            let answer = project.type_of(request(&fixture, marker, depth));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{marker}: {answer:?}")
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, summary);
            assert!(result.issues.is_empty(), "{marker}: {:?}", result.issues);
        }
    }
}

//! Native declared identities survive cycles and structurally equal symbols.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeQuestion, TypeRef,
    TypeRequest,
};
const SOURCE: &str = include_str!("../fixtures/types/native-recursive-types/types.ts");
const CONSUMER: &str = include_str!("../fixtures/types/native-recursive-types/consumer.ts");
const CONFIG: &str = include_str!("../fixtures/types/native-recursive-types/tsconfig.json");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("tsconfig.json", CONFIG),
        ("types.ts", SOURCE),
        ("consumer.ts", CONSUMER),
    ])
}
fn query(fixture: &Fixture, marker: &str) -> TypeRequest {
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("consumer.ts"),
        position: position(CONSUMER, CONSUMER.find(marker).unwrap()),
    });
    request.expand = true;
    request
}

#[test]
fn self_references_reuse_the_root_graph_id_without_display_truncation() {
    let fixture = fixture();
    let project = fixture.load_config("tsconfig.json");
    for depth in [1, 3, 32] {
        let mut request = query(&fixture, "ImportedRecursive");
        request.depth = depth;
        let answer = project.type_of(request).unwrap();
        let TypeAnswer::Selected(result) = answer else {
            panic!("the named interface is selected")
        };
        assert_eq!(result.target.kind, TypeQuestion::Declared);
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(result.description.summary, "Recursive");
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        assert_eq!(graph.root, "t0");
        assert_eq!(graph.nodes.len(), 2);
        assert!(
            !graph.truncated,
            "a reused semantic identity is a cycle, not display elision"
        );
        let TypeNodeData::Object { members } = &graph.nodes[0].data else {
            panic!("the interface stays an object")
        };
        let properties = members.properties.as_ref().unwrap();
        assert_eq!(properties[0].read_type, TypeRef::Ref { r#ref: "t0".into() });
        assert_eq!(properties[1].read_type, TypeRef::Ref { r#ref: "t1".into() });
        assert_eq!(
            graph.nodes[1].data,
            TypeNodeData::Primitive {
                name: PrimitiveName::String
            }
        );
    }
}

#[test]
fn distinct_nominal_symbols_keep_distinct_nodes_while_exact_references_share() {
    let fixture = fixture();
    let project = fixture.load_config("tsconfig.json");
    for depth in [2, 3, 32] {
        let mut request = query(&fixture, "ImportedHolder");
        request.depth = depth;
        let answer = project.type_of(request).unwrap();
        let TypeAnswer::Selected(result) = answer else {
            panic!("the named holder is selected")
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(result.description.summary, "Holder");
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        assert_eq!(graph.nodes.len(), 4);
        assert!(!graph.truncated);
        let TypeNodeData::Object { members } = &graph.nodes[0].data else {
            panic!("the holder stays an object")
        };
        let properties = members.properties.as_ref().unwrap();
        assert_eq!(
            properties
                .iter()
                .map(|property| property.read_type.clone())
                .collect::<Vec<_>>(),
            vec![
                TypeRef::Ref { r#ref: "t1".into() },
                TypeRef::Ref { r#ref: "t2".into() },
                TypeRef::Ref { r#ref: "t1".into() },
            ]
        );
        let site = position(
            SOURCE,
            SOURCE.find("interface Alpha").unwrap() + "interface ".len(),
        );
        assert_eq!(
            graph.nodes[1].summary,
            format!("Alpha (types.ts:{}:{})", site.line, site.column)
        );
        let site = position(
            SOURCE,
            SOURCE.find("interface Beta").unwrap() + "interface ".len(),
        );
        assert_eq!(
            graph.nodes[2].summary,
            format!("Beta (types.ts:{}:{})", site.line, site.column)
        );
        for object in &graph.nodes[1..3] {
            let TypeNodeData::Object { members } = &object.data else {
                panic!("the distinct symbol has an object node")
            };
            assert_eq!(
                members.properties.as_ref().unwrap()[0].read_type,
                TypeRef::Ref { r#ref: "t3".into() }
            );
            assert!(
                object.alias.is_none(),
                "a nominal interface name is separate from type alias metadata"
            );
        }
        assert_eq!(
            graph.nodes[3].data,
            TypeNodeData::Primitive {
                name: PrimitiveName::String
            }
        );
    }
}

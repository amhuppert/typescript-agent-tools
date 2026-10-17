//! A natural partial graph proves recovery inside one evaluator invocation.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::SemanticFailureReason;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeKey, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};
const SOURCE: &str = include_str!("../fixtures/types/native-inference-lifecycle/cases.ts");

#[test]
fn failed_inference_retires_its_session_without_hiding_later_relations() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-inference-lifecycle/tsconfig.json"),
        ),
        ("cases.ts", SOURCE),
    ]);
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find("recoveryEnvelope:").unwrap()),
    });
    request.expand = true;
    request.depth = 3;
    let answer = fixture.load().type_of(request);
    let Ok(TypeAnswer::Selected(result)) = &answer else {
        panic!("recoverable flow dependency: {answer:?}");
    };
    assert_eq!(result.outcome, TypeOutcome::Partial, "{result:?}");
    assert_eq!(result.issues.len(), 1, "{result:?}");
    assert_eq!(result.issues[0].reason, SemanticFailureReason::FlowRequired);
    assert_eq!(result.issues[0].location.file, "cases.ts");
    assert_eq!(
        result.issues[0].location.start,
        position(
            SOURCE,
            SOURCE.find("narrowed = value").unwrap() + "narrowed = ".len()
        )
    );
    let graph = result.description.expansion.as_ref().unwrap();
    assert!(!graph.truncated);
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    let TypeNodeData::Object { members } = &root.data else {
        panic!("{root:?}");
    };
    let properties = members.properties.as_ref().unwrap();
    assert_eq!(
        properties
            .iter()
            .map(|property| &property.key)
            .collect::<Vec<_>>(),
        ["failed", "healthy", "repeated"]
            .map(|name| TypeKey::String { name: name.into() })
            .iter()
            .collect::<Vec<_>>(),
        "the healthy relation must run after the failed relation in this evaluator"
    );
    let property = |name: &str| {
        properties
            .iter()
            .find(|p| p.key == TypeKey::String { name: name.into() })
            .unwrap()
    };
    let TypeRef::Ref { r#ref: healthy } = &property("healthy").read_type else {
        panic!("{properties:?}");
    };
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == *healthy)
            .unwrap()
            .data,
        TypeNodeData::Literal {
            base: PrimitiveName::String,
            value: "chosen".into()
        }
    );
    let failed = &property("failed").read_type;
    assert_eq!(
        failed,
        &property("repeated").read_type,
        "the original failed property question remains shared"
    );
    let TypeRef::Ref { r#ref: failed } = failed else {
        panic!("{properties:?}");
    };
    assert!(matches!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == *failed)
            .unwrap()
            .data,
        TypeNodeData::Unsupported { .. }
    ));
}

//! Source-written function symbols have a separate native serialization rule.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const FUNCTIONS: &str = include_str!("../fixtures/types/function-symbols/functions.ts");
const CONSUMER: &str = include_str!("../fixtures/types/function-symbols/consumer.ts");

#[test]
fn visited_function_types_preserve_nonlocal_symbols_and_local_recursive_placeholders() {
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/function-symbols/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/function-symbols/loose.json"),
        ),
        ("functions.ts", FUNCTIONS),
        ("consumer.ts", CONSUMER),
    ]);
    // The native arrow control remains a prerequisite for the separate
    // arrow-function construction owner, rather than a spelling inference.
    let cases = [
        ("functions.ts", "top", "() => typeof top"),
        ("functions.ts", "privateTop", "() => typeof privateTop"),
        ("functions.ts", "local", "() => ..."),
        ("functions.ts", "member", "() => typeof member"),
        ("functions.ts", "escaped", "() => typeof \\u0052aw"),
        ("consumer.ts", "renamed", "() => typeof top"),
        ("consumer.ts", "importedEscaped", "() => typeof \\u0052aw"),
    ];
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (file, marker, summary) in cases {
            let source = if file == "functions.ts" {
                FUNCTIONS
            } else {
                CONSUMER
            };
            let marker = format!("/*#{marker}*/");
            let offset = source.find(&marker).unwrap() + marker.len();
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path(file),
                    position: position(source, offset),
                });
                request.expand = expand;
                request.depth = 3;
                let answer = project.type_of(request);
                assert!(
                    matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
                    "{config} {marker}: an established recursive function stays healthy: {answer:?}"
                );
                let TypeAnswer::Selected(result) = answer.unwrap() else {
                    unreachable!()
                };
                assert_eq!(result.description.summary, summary, "{config} {marker}");
                assert!(result.issues.is_empty());
                if let Some(graph) = result.description.expansion {
                    let root = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == graph.root)
                        .unwrap();
                    let TypeNodeData::Function { members } = &root.data else {
                        panic!("{marker}: actual call signature classifies the function")
                    };
                    let calls = members.call_signatures.as_ref().unwrap();
                    assert_eq!(calls.len(), 1);
                    assert_eq!(calls[0].return_type, TypeRef::Ref { r#ref: graph.root });
                }
            }
        }
    }
}

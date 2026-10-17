//! Ordinary parameter value/cache questions share their native canonical symbol.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn signature_parameter_and_inferred_return_share_canonical_parameter_type_identity() {
    let source = include_str!("../fixtures/types/native-parameter-reads/parameters.ts");
    let config = include_str!("../fixtures/types/native-parameter-reads/tsconfig.json");
    let fixture = Fixture::new(&[("tsconfig.json", config), ("parameters.ts", source)]);
    let loaded = fixture.load();
    for (name, summary) in [
        ("shared", "(value: { n: number; }) => { n: number; }"),
        (
            "optional",
            "(value?: number | undefined) => number | undefined",
        ),
        ("initialized", "(value?: number) => number"),
    ] {
        let marker = format!("function {name}(");
        let mut request = TypeRequest::new(TypeQuery::At {
            file: fixture.path("parameters.ts"),
            position: position(source, source.find(&marker).unwrap() + "function ".len()),
        });
        request.expand = true;
        request.depth = 3;
        let answer = loaded.type_of(request);
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!("a native parameter read establishes the annotated returned type: {answer:?}")
        };
        assert_eq!(result.outcome, TypeOutcome::Found, "{result:?}");
        assert!(result.issues.is_empty());
        assert_eq!(result.description.summary, summary);
        let graph = result.description.expansion.as_ref().unwrap();
        let TypeNodeData::Function { members } = &graph.nodes[0].data else {
            panic!("the selected function has an actual signature")
        };
        let signatures = members.call_signatures.as_ref().unwrap();
        assert_eq!(signatures.len(), 1);
        assert_eq!(signatures[0].parameters.len(), 1);
        assert_eq!(
            signatures[0].parameters[0].value_type, signatures[0].return_type,
            "same canonical parameter symbol Type property, including an anonymous annotation"
        );
        assert!(!graph.truncated);
    }
}

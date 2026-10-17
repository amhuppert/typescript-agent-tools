//! Optional tuple display and raw graph arguments ask different native questions.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/tuple-optionality/cases.ts");

#[test]
fn optional_tuple_display_removes_only_missing_without_changing_raw_graph_arguments() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/tuple-optionality/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/tuple-optionality/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/tuple-optionality/exact.json"),
        ),
    ]);
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        let strict = config != "loose.json";
        let exact = config == "exact.json";
        let optional = if strict && !exact {
            "string | undefined"
        } else {
            "string"
        };
        let explicit = if strict {
            "string | undefined"
        } else {
            "string"
        };
        for (marker, summary) in [
            (
                "unnamed",
                if strict && !exact {
                    "[(string | undefined)?]"
                } else {
                    "[string?]"
                }
                .to_owned(),
            ),
            ("named", format!("[value?: {optional}]")),
            (
                "explicitUnnamed",
                if strict {
                    "[(string | undefined)?]"
                } else {
                    "[string?]"
                }
                .to_owned(),
            ),
            ("explicitNamed", format!("[value?: {explicit}]")),
            ("required", format!("[{explicit}]")),
            ("readonly", format!("readonly [value?: {optional}]")),
        ] {
            let tag = format!("/*#{marker}*/");
            for (expand, depth) in [(false, 0), (true, 0), (true, 1)] {
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
                    },
                    expand,
                    depth,
                });
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}/{depth}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert!(result.issues.is_empty());
                assert_eq!(result.description.summary, summary, "{config}/{marker}");
                if expand && depth == 1 {
                    let graph = result.description.expansion.unwrap();
                    let root = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == graph.root)
                        .unwrap();
                    let TypeNodeData::Tuple {
                        elements, readonly, ..
                    } = &root.data
                    else {
                        panic!("{root:?}");
                    };
                    assert_eq!(*readonly, marker == "readonly");
                    assert_eq!(elements.len(), 1);
                    assert_eq!(elements[0].optional, marker != "required");
                    let arguments = root.type_arguments.as_ref().unwrap();
                    assert_eq!(arguments.len(), 1);
                    assert_eq!(arguments[0], elements[0].value_type);
                    let TypeRef::Ref { r#ref } = &arguments[0] else {
                        panic!("expected actual argument edge");
                    };
                    let raw = graph.nodes.iter().find(|node| node.id == *r#ref).unwrap();
                    assert_eq!(raw.summary, explicit, "{config}/{marker}/raw");
                    assert_eq!(matches!(raw.data, TypeNodeData::Union { .. }), strict);
                }
            }
        }
    }
}

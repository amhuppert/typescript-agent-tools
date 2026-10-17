//! Graph labels stay unambiguous without changing native root summaries.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{
    TypeAnswer, TypeKey, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/graph-labels/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        ("a.ts", include_str!("../fixtures/types/graph-labels/a.ts")),
        ("b.ts", include_str!("../fixtures/types/graph-labels/b.ts")),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/graph-labels/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/graph-labels/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/graph-labels/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/graph-labels/exact.json"),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str, depth: usize) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand: true,
        depth,
    }
}

#[test]
fn aliases_and_nominals_use_actual_declarations_in_both_discovery_orders() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [("forward", "Forward"), ("reverse", "Reverse")] {
            let answer = project.type_of(request(&fixture, marker, 2));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{config}/{marker}: {answer:?}");
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, summary);
            let graph = result.description.expansion.unwrap();
            let TypeNodeData::Object { members } = &graph.nodes[0].data else {
                panic!("{graph:?}");
            };
            let property = |name: &str| {
                members
                    .properties
                    .as_ref()
                    .unwrap()
                    .iter()
                    .find(|p| p.key == TypeKey::String { name: name.into() })
                    .unwrap()
            };
            let node = |name: &str| {
                let TypeRef::Ref { r#ref } = &property(name).read_type else {
                    panic!("property must have a graph node");
                };
                graph.nodes.iter().find(|node| &node.id == r#ref).unwrap()
            };
            for (name, expected, origin) in [
                ("first", "Same<string> (a.ts:1:13)", "a.ts"),
                ("second", "Same<number> (b.ts:1:13)", "b.ts"),
                ("third", "Same<number> (a.ts:1:13)", "a.ts"),
            ] {
                let node = node(name);
                assert_eq!(node.summary, expected);
                assert!(!node.summary_truncated);
                let alias = node.alias.as_ref().unwrap();
                assert_eq!(alias.name, "Same");
                assert_eq!(alias.origins[0].file, origin);
                assert_eq!(alias.origins[0].start.line, 1);
                assert_eq!(alias.origins[0].start.column, 13);
                assert_eq!(alias.origins[0].end.column, 17);
            }
            assert_eq!(node("nominalA").summary, "Named (a.ts:2:18)");
            assert_eq!(node("nominalB").summary, "Named (b.ts:2:18)");
            assert_eq!(property("first").read_type, property("repeated").read_type);
            assert_ne!(property("first").read_type, property("third").read_type);
            assert_ne!(property("second").read_type, property("third").read_type);
        }
    }
}

#[test]
fn omitted_edges_use_the_same_qualified_label_without_a_fabricated_id() {
    let fixture = fixture();
    let answer = fixture
        .load()
        .type_of(request(&fixture, "array", 0))
        .unwrap();
    let TypeAnswer::Selected(result) = answer else {
        panic!("{answer:?}");
    };
    assert_eq!(result.description.summary, "Same<string>[]");
    let graph = result.description.expansion.unwrap();
    assert_eq!(graph.nodes.len(), 1);
    let TypeNodeData::Array { element, .. } = &graph.nodes[0].data else {
        panic!("{graph:?}");
    };
    assert!(
        matches!(element, TypeRef::Omitted { summary, summary_truncated: false, .. } if summary == "Same<string> (a.ts:1:13)")
    );
}

#[test]
fn qualification_keeps_its_exact_suffix_at_the_summary_limit() {
    // Full graph label = N + 15 bytes. The root's native token still fits.
    for length in [496, 497, 498] {
        let name = "A".repeat(length);
        let source = format!("type {name} = {{ value: string }};\ndeclare const value: {name};\n");
        let fixture = Fixture::new(&[
            ("cases.ts", &source),
            (
                "tsconfig.json",
                "{\"compilerOptions\":{\"noLib\":true},\"files\":[\"cases.ts\"]}",
            ),
        ]);
        let answer = fixture
            .load()
            .type_of(TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(&source, source.rfind("value:").unwrap()),
                },
                expand: true,
                depth: 0,
            })
            .unwrap();
        let TypeAnswer::Selected(result) = answer else {
            panic!("{answer:?}");
        };
        assert_eq!(result.description.summary, name);
        let graph = result.description.expansion.unwrap();
        assert_eq!(
            graph.nodes[0].summary,
            if length <= 497 {
                format!("{name} (cases.ts:1:6)")
            } else {
                "… (cases.ts:1:6)".into()
            }
        );
        assert_eq!(graph.nodes[0].summary_truncated, length > 497);
        assert!(graph.nodes[0].summary.len() <= 512);
        assert_eq!(graph.nodes[0].alias.as_ref().unwrap().name, name);
    }
}

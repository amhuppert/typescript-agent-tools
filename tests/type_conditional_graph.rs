//! Q4 binding metadata over the pinned inline conditional controls.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeExpansion, TypeNodeData, TypeOperator, TypeOutcome, TypeQuery,
    TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/native-conditional-types/cases.ts");

fn graph(marker: &str, config: &str, depth: usize) -> TypeExpansion {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-conditional-types/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-conditional-types/tsconfig.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-conditional-types/exact.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-conditional-types/loose.json"),
        ),
    ]);
    let tag = format!("/*#{marker}*/");
    let answer = fixture.load_config(config).type_of(TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand: true,
        depth,
    });
    let Ok(TypeAnswer::Selected(result)) = answer else {
        panic!("{marker}/{config}: {answer:?}");
    };
    assert_eq!(
        result.outcome,
        TypeOutcome::Found,
        "{marker}/{config}: {:?}",
        result.issues
    );
    assert!(result.issues.is_empty());
    result.description.expansion.unwrap()
}

#[test]
fn conditional_descriptors_keep_their_bound_parameter_and_constraint() {
    for config in ["tsconfig.json", "exact.json", "loose.json"] {
        for marker in ["inlineConstrained", "inlineTupleConstrained"] {
            let graph = graph(marker, config, 3);
            let parameters = graph
                .nodes
                .iter()
                .find_map(|node| match &node.data {
                    TypeNodeData::Operator {
                        operator: TypeOperator::Conditional,
                        parameters,
                        omitted_parameters: None,
                        ..
                    } => Some(parameters),
                    _ => None,
                })
                .expect("conditional return node");
            assert_eq!(parameters.len(), 1, "{marker}/{config}: {graph:?}");
            assert_eq!(parameters[0].name, "U");
            let Some(TypeRef::Ref { r#ref }) = &parameters[0].constraint else {
                panic!("missing constraint: {parameters:?}");
            };
            assert!(graph.nodes.iter().any(|node| &node.id == r#ref
                && matches!(
                    node.data,
                    TypeNodeData::Primitive {
                        name: PrimitiveName::String
                    }
                )));
            assert!(parameters[0].default.is_none());
        }
    }
}

#[test]
fn nested_conditionals_keep_binders_in_their_own_scope() {
    for config in ["tsconfig.json", "exact.json", "loose.json"] {
        let graph = graph("inlineNested", config, 3);
        let mut names: Vec<Vec<_>> = graph
            .nodes
            .iter()
            .filter_map(|node| match &node.data {
                TypeNodeData::Operator {
                    operator: TypeOperator::Conditional,
                    parameters,
                    ..
                } => Some(parameters.iter().map(|p| p.name.as_str()).collect()),
                _ => None,
            })
            .collect();
        names.sort();
        assert_eq!(names, [Vec::<&str>::new(), vec!["W"]]);
    }
}

#[test]
fn depth_boundary_reports_unshown_conditional_parameters() {
    let graph = graph("inlineConstrained", "tsconfig.json", 1);
    let conditional = graph
        .nodes
        .iter()
        .find(|node| {
            matches!(
                node.data,
                TypeNodeData::Operator {
                    operator: TypeOperator::Conditional,
                    ..
                }
            )
        })
        .unwrap();
    let omitted = conditional.omitted.as_ref().unwrap();
    assert!(omitted.fields.iter().any(|field| field == "parameters"));
    assert!(
        matches!(&conditional.data, TypeNodeData::Operator { parameters, omitted_parameters: Some(1), omitted_operands: Some(4), .. } if parameters.is_empty())
    );
}

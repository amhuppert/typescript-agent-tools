//! Declared constraints and recursively resolved bases remain distinct native questions.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/base-constraints/cases.ts");
const STRICT: &str = include_str!("../fixtures/types/base-constraints/tsconfig.json");
const LOOSE: &str = include_str!("../fixtures/types/base-constraints/loose.json");

fn query(fixture: &Fixture, marker: &str) -> TypeQuery {
    let tag = format!("/*#{marker}*/");
    TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
    }
}

#[test]
fn parameter_constraint_graphs_keep_raw_symbols_after_resolving_actual_bases() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        ("tsconfig.json", STRICT),
        ("loose.json", LOOSE),
    ]);
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, child_name, grandchild_summary) in [
            ("literal", "U", Some("\"ready\"")),
            ("free", "U", None),
            ("object", "U", Some("{ value: string; }")),
        ] {
            let mut request = TypeRequest::new(query(&fixture, marker));
            request.expand = true;
            request.depth = 3;
            let answer = project.type_of(request);
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "native base resolution must preserve a healthy symbolic constraint ({config}, {marker}): {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            assert_eq!(result.description.summary, "T");
            let graph = result.description.expansion.unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            let TypeNodeData::TypeParameter {
                constraint: Some(TypeRef::Ref { r#ref }),
                ..
            } = &root.data
            else {
                panic!("{root:?}")
            };
            let child = graph.nodes.iter().find(|node| node.id == *r#ref).unwrap();
            assert_eq!(child.summary, child_name);
            let TypeNodeData::TypeParameter { constraint, .. } = &child.data else {
                panic!("{child:?}")
            };
            match grandchild_summary {
                None => assert!(constraint.is_none()),
                Some(expected) => {
                    let Some(TypeRef::Ref { r#ref }) = constraint else {
                        panic!("{child:?}")
                    };
                    assert_eq!(
                        graph
                            .nodes
                            .iter()
                            .find(|node| node.id == *r#ref)
                            .unwrap()
                            .summary,
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn reference_and_recursive_structural_constraints_do_not_demand_unrelated_member_children() {
    let fixture = Fixture::new(&[("cases.ts", SOURCE), ("tsconfig.json", STRICT)]);
    let project = fixture.load();
    // The native spelling is Box<number>; named Q4 graph labels also retain
    // the actual declaration qualifier required by the presentation contract.
    for (marker, expected) in [
        ("reference", "Box<number> (cases.ts:6:11)"),
        ("recursive", "{ next: T; }"),
    ] {
        let mut request = TypeRequest::new(query(&fixture, marker));
        request.expand = true;
        request.depth = 1;
        let answer = project.type_of(request);
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "native constraint must establish its structural base ({marker}): {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        let TypeNodeData::TypeParameter {
            constraint: Some(TypeRef::Ref { r#ref }),
            ..
        } = &root.data
        else {
            panic!("{root:?}")
        };
        assert_eq!(
            graph
                .nodes
                .iter()
                .find(|node| node.id == *r#ref)
                .unwrap()
                .summary,
            expected
        );
    }
}

#[test]
fn consumed_circular_constraint_recovery_stays_invalid_after_a_valid_parameter_root() {
    let fixture = Fixture::new(&[("cases.ts", SOURCE), ("tsconfig.json", STRICT)]);
    let project = fixture.load();
    for marker in ["direct", "mutual"] {
        for expand in [false, true] {
            let mut request = TypeRequest::new(query(&fixture, marker));
            request.expand = expand;
            request.depth = 0;
            assert!(
                matches!(project.type_of(request), Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found)
            );
        }
        let mut request = TypeRequest::new(query(&fixture, marker));
        request.expand = true;
        request.depth = 1;
        let answer = project.type_of(request);
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error))
            if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::CircularType.into()),
            "native CircularConstraint is consumed recovery, not an absent healthy constraint ({marker}): {answer:?}"
        );
        if marker == "direct" {
            let Err(AnalysisError::Semantic(error)) = answer else {
                unreachable!()
            };
            assert_eq!(error.path, Some(fixture.path("cases.ts")));
            assert_eq!(
                error.position,
                Some(position(
                    SOURCE,
                    SOURCE.find("DirectCycle<T extends ").unwrap() + "DirectCycle<T extends ".len()
                ))
            );
        }
    }
}

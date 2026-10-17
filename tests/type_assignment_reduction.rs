//! Final joins and fallback reductions keep the exact native read identity.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/assignment-reduction/flow.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("flow.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/assignment-reduction/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/assignment-reduction/loose.json"),
        ),
    ])
}

fn answer(fixture: &Fixture, config: &str, marker: &str) -> Result<TypeAnswer, AnalysisError> {
    let comment = format!("/*{marker}*/ ");
    let offset = SOURCE.find(&comment).unwrap() + comment.len();
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("flow.ts"),
        position: position(SOURCE, offset),
    });
    request.expand = true;
    request.depth = 2;
    fixture.load_config(config).type_of(request)
}

fn found(fixture: &Fixture, config: &str, marker: &str, summary: &str) -> TypeNodeData {
    let answer = answer(fixture, config, marker);
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
        "{config}: {marker} must complete with its established native identity: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.description.summary, summary, "{config}: {marker}");
    assert!(result.issues.is_empty());
    let graph = result.description.expansion.unwrap();
    graph
        .nodes
        .into_iter()
        .find(|node| node.id == graph.root)
        .unwrap()
        .data
}

#[test]
fn scalar_branch_join_and_union_assignment_restore_the_declared_native_union() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        for marker in ["scalar-join", "union-assigned"] {
            assert!(matches!(
                found(&fixture, config, marker, "string | number"),
                TypeNodeData::Union { .. }
            ));
        }
    }
}

#[test]
fn fresh_boolean_reduction_join_restores_the_native_boolean_identity() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        assert_eq!(
            found(&fixture, config, "boolean-join", "boolean"),
            TypeNodeData::Primitive {
                name: typescript_parser::types::PrimitiveName::Boolean
            }
        );
    }
}

#[test]
fn rejected_assignment_fallback_is_declared_type_and_does_not_consume_unused_rhs() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        found(&fixture, config, "wrong-assigned", "string | number");
        assert_eq!(
            found(&fixture, config, "unused-assigned", "number"),
            TypeNodeData::Primitive {
                name: typescript_parser::types::PrimitiveName::Number
            }
        );
    }
}

#[test]
fn actually_demanded_assignment_rhs_preserves_its_original_missing_symbol() {
    let fixture = fixture();
    let offset = SOURCE.find("value = Missing;").unwrap() + "value = ".len();
    let expected = position(SOURCE, offset);
    let path = fixture.path("flow.ts").canonicalize().unwrap();
    for config in ["tsconfig.json", "loose.json"] {
        let answer = answer(&fixture, config, "missing-assigned");
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error))
            if error.kind == SemanticErrorKind::InvalidSemantic
                && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
                && error.path.as_deref() == Some(path.as_path())
                && error.position == Some(expected)),
            "{config}: a demanded RHS must remain its actual failure: {answer:?}"
        );
    }
}

//! Final native declared-parameter selection and exact display/source identity.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeQuestion, TypeRef,
    TypeRequest, TypeResult,
};

const SOURCE: &str = include_str!("../fixtures/types/native-parameter-selection/parameters.ts");
const CONFIG: &str = include_str!("../fixtures/types/native-parameter-selection/tsconfig.json");

fn fixture() -> Fixture {
    Fixture::new(&[("tsconfig.json", CONFIG), ("parameters.ts", SOURCE)])
}

fn request(fixture: &Fixture, marker: &str, expand: bool, depth: usize) -> TypeRequest {
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("parameters.ts"),
            position: position(SOURCE, SOURCE.find(marker).unwrap()),
        },
        expand,
        depth,
    }
}

fn selected(fixture: &Fixture, marker: &str, expand: bool, depth: usize) -> TypeResult {
    let answer = fixture
        .load()
        .type_of(request(fixture, marker, expand, depth));
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(_))),
        "the final parameter question must establish {marker}: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert!(result.issues.is_empty());
    result
}

#[test]
fn own_parameter_declarations_select_declared_and_type_uses_keep_their_distinct_question() {
    let fixture = fixture();
    for (marker, question, name, width) in [
        (
            "T extends MissingConstraint",
            TypeQuestion::Declared,
            "T",
            1,
        ),
        ("T extends number", TypeQuestion::Declared, "T", 1),
        ("U> { first", TypeQuestion::Declared, "U", 1),
        ("U extends string", TypeQuestion::Declared, "U", 1),
        ("T; }\ninterface Second", TypeQuestion::Type, "T", 1),
    ] {
        let result = selected(&fixture, marker, true, 0);
        assert_eq!(result.target.kind, question);
        assert_eq!(result.description.summary, name);
        assert_eq!(result.target.location.file, "parameters.ts");
        assert_eq!(
            result.target.location.start,
            position(SOURCE, SOURCE.find(marker).unwrap())
        );
        assert_eq!(
            result.target.location.end,
            position(SOURCE, SOURCE.find(marker).unwrap() + width)
        );
        let graph = result.description.expansion.unwrap();
        assert_eq!(graph.nodes.len(), 1);
        assert!(
            matches!(&graph.nodes[0].data, TypeNodeData::TypeParameter { name: actual, .. } if actual == name)
        );
        assert!(graph.nodes[0].alias.is_none());
    }
}

#[test]
fn escaped_parameter_names_keep_decoded_graph_identity_and_native_raw_summary_spelling() {
    let fixture = fixture();
    for (marker, question, width) in [
        ("\\u0056; }", TypeQuestion::Type, 6),
        ("\\u0056 extends string", TypeQuestion::Declared, 6),
        ("V> { other", TypeQuestion::Declared, 1),
    ] {
        let result = selected(&fixture, marker, true, 3);
        assert_eq!(result.target.kind, question);
        assert_eq!(
            result.description.summary, "\\u0056",
            "native getNameOfSymbolAsWritten is separate from the decoded parameter identity"
        );
        assert_eq!(
            result.target.location.start,
            position(SOURCE, SOURCE.find(marker).unwrap())
        );
        assert_eq!(
            result.target.location.end,
            position(SOURCE, SOURCE.find(marker).unwrap() + width)
        );
        let graph = result.description.expansion.unwrap();
        let TypeNodeData::TypeParameter {
            name,
            constraint: Some(TypeRef::Ref { r#ref: constraint }),
            default: Some(TypeRef::Ref { r#ref: default }),
        } = &graph.nodes[0].data
        else {
            panic!("{graph:?}")
        };
        assert_eq!(name, "V");
        assert_eq!(
            graph
                .nodes
                .iter()
                .find(|node| node.id == *constraint)
                .unwrap()
                .data,
            TypeNodeData::Primitive {
                name: PrimitiveName::String
            }
        );
        assert_eq!(
            graph
                .nodes
                .iter()
                .find(|node| node.id == *default)
                .unwrap()
                .data,
            TypeNodeData::Literal {
                base: PrimitiveName::String,
                value: "done".into()
            }
        );
    }
    for (marker, question, width) in [
        ("W extends string", TypeQuestion::Declared, 1),
        ("\\u0057> { second", TypeQuestion::Declared, 6),
        ("\\u0057; }", TypeQuestion::Type, 6),
    ] {
        let result = selected(&fixture, marker, true, 3);
        assert_eq!(result.target.kind, question);
        assert_eq!(
            result.description.summary, "W",
            "first own declaration decides the raw display spelling"
        );
        assert_eq!(
            result.target.location.end,
            position(SOURCE, SOURCE.find(marker).unwrap() + width)
        );
        let graph = result.description.expansion.unwrap();
        assert!(
            matches!(&graph.nodes[0].data, TypeNodeData::TypeParameter { name, .. } if name == "W")
        );
    }
}

#[test]
fn merged_own_parameter_anchors_share_the_complete_lazy_canonical_parameter_decision() {
    let fixture = fixture();
    for marker in [
        "U> { first",
        "U extends string",
        "U; }\ninterface Joined",
        "U; }\ninterface Escaped",
    ] {
        let result = selected(&fixture, marker, true, 3);
        assert_eq!(result.description.summary, "U");
        let graph = result.description.expansion.unwrap();
        assert!(!graph.truncated);
        let TypeNodeData::TypeParameter {
            name,
            constraint: Some(TypeRef::Ref { r#ref: constraint }),
            default: Some(TypeRef::Ref { r#ref: default }),
        } = &graph.nodes[0].data
        else {
            panic!("{graph:?}")
        };
        assert_eq!(name, "U");
        assert_eq!(
            graph
                .nodes
                .iter()
                .find(|node| node.id == *constraint)
                .unwrap()
                .data,
            TypeNodeData::Primitive {
                name: PrimitiveName::String
            }
        );
        assert_eq!(
            graph
                .nodes
                .iter()
                .find(|node| node.id == *default)
                .unwrap()
                .data,
            TypeNodeData::Literal {
                base: PrimitiveName::String,
                value: "ready".into()
            }
        );
    }
    let loaded = fixture.load();
    for expand in [false, true] {
        let answer = loaded.type_of(request(&fixture, "T extends MissingConstraint", expand, 0));
        assert!(
            matches!(answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found)
        );
    }
    let answer = loaded.type_of(request(&fixture, "T extends MissingConstraint", true, 3));
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
        "admitted parameter child recovery must fail at its source: {answer:?}"
    );
    let Err(AnalysisError::Semantic(error)) = answer else {
        unreachable!()
    };
    assert_eq!(error.path, Some(fixture.path("parameters.ts")));
    assert_eq!(
        error.position,
        Some(position(SOURCE, SOURCE.find("MissingConstraint").unwrap()))
    );
}

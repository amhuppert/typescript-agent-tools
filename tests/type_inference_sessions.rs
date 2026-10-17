//! Final overload consumers for native contextual-inference decisions.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest,
};

const DEFINITIONS: &str =
    include_str!("../fixtures/types/native-inference-sessions/definitions.ts");
const SITES: &str = include_str!("../fixtures/types/native-inference-sessions/sites.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-inference-sessions/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-inference-sessions/loose.json"),
        ),
        ("definitions.ts", DEFINITIONS),
        ("sites.ts", SITES),
    ])
}

fn selected(name: &str, expected: &str) {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(profile);
        let mut request = TypeRequest::new(TypeQuery::At {
            file: fixture.path("sites.ts"),
            position: position(SITES, SITES.find(&format!("{name}Result =")).unwrap()),
        });
        request.expand = true;
        request.depth = 2;
        let answer = project.type_of(request);
        let Ok(TypeAnswer::Selected(result)) = &answer else {
            panic!("{profile}:{name}: actual inference must complete: {answer:?}");
        };
        assert_eq!(
            result.outcome,
            TypeOutcome::Found,
            "{profile}:{name}: {result:?}"
        );
        assert_eq!(
            result.description.summary,
            format!("\"{expected}\""),
            "{profile}:{name}"
        );
        assert!(result.issues.is_empty(), "{profile}:{name}");
        let graph = result.description.expansion.as_ref().unwrap();
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        assert_eq!(
            root.data,
            TypeNodeData::Literal {
                base: PrimitiveName::String,
                value: expected.into()
            },
            "{profile}:{name}"
        );
    }
}

fn missing(name: &str, token: &str) {
    let fixture = fixture();
    let path = fixture.path("definitions.ts").canonicalize().unwrap();
    let expected = position(DEFINITIONS, DEFINITIONS.find(token).unwrap());
    for profile in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(profile);
        let request = TypeRequest::new(TypeQuery::At {
            file: fixture.path("sites.ts"),
            position: position(SITES, SITES.find(&format!("{name}Result =")).unwrap()),
        });
        let answer = project.type_of(request);
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error))
            if error.kind == SemanticErrorKind::InvalidSemantic
                && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
                && error.path.as_deref() == Some(path.as_path())
                && error.position == Some(expected)),
            "{profile}:{name}: retain the actually consumed inference input: {answer:?}"
        );
    }
}

#[test]
fn parameter_priority_uses_native_contextual_inference() {
    selected("parameterPriority", "chosen");
}

#[test]
fn return_combination_uses_native_contextual_inference() {
    selected("returnCombination", "chosen");
}

#[test]
fn return_constraint_filter_uses_native_contextual_inference() {
    selected("returnConstraintFilter", "chosen");
}

#[test]
fn constraint_reject_uses_native_contextual_inference() {
    selected("constraintReject", "same");
}

#[test]
fn dependent_default_uses_native_contextual_inference() {
    selected("dependentDefault", "same");
}

#[test]
fn forward_default_uses_native_contextual_inference() {
    selected("forwardDefault", "chosen");
}

#[test]
fn unused_default_uses_native_contextual_inference() {
    selected("unusedDefault", "chosen");
}

#[test]
fn needed_default_uses_native_contextual_inference() {
    missing("neededDefault", "MissingNeededDefault");
}

#[test]
fn missing_constraint_uses_native_contextual_inference() {
    missing("missingConstraint", "MissingInferenceConstraint");
}

#[test]
fn primitive_literal_uses_native_contextual_inference() {
    selected("primitiveLiteral", "chosen");
}

#[test]
fn const_literal_uses_native_contextual_inference() {
    selected("constLiteral", "chosen");
}

#[test]
fn nested_object_uses_native_contextual_inference() {
    selected("nestedObject", "chosen");
}

#[test]
fn reference_arguments_uses_native_contextual_inference() {
    selected("referenceArguments", "chosen");
}

#[test]
fn callback_inference_uses_native_contextual_inference() {
    selected("callbackInference", "chosen");
}

#[test]
fn candidate_conflict_uses_native_contextual_inference() {
    selected("candidateConflict", "same");
}

#[test]
fn union_matching_uses_native_contextual_inference() {
    selected("unionMatching", "chosen");
}

#[test]
fn recursive_constraint_uses_native_contextual_inference() {
    selected("recursiveConstraint", "chosen");
}

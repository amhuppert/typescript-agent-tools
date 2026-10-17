//! Final native calls preserve quick initializer and checked body questions.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::project::Project;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeQuestion, TypeRef,
    TypeRequest, TypeResult,
};

const FUNCTIONS: &str = include_str!("../fixtures/types/native-calls/functions.ts");
const CONSUMER: &str = include_str!("../fixtures/types/native-calls/consumer.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-calls/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-calls/loose.json"),
        ),
        ("functions.ts", FUNCTIONS),
        ("consumer.ts", CONSUMER),
    ])
}

fn answer(project: &Project, fixture: &Fixture, name: &str) -> Result<TypeAnswer, AnalysisError> {
    let offset = CONSUMER.find(&format!("{name},")).unwrap();
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("consumer.ts"),
        position: position(CONSUMER, offset),
    });
    request.expand = true;
    request.depth = 3;
    project.type_of(request)
}

fn selected(project: &Project, fixture: &Fixture, name: &str, summary: &str) -> TypeResult {
    let answer = answer(project, fixture, name);
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
        "{name} must establish its native call result: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.target.kind, TypeQuestion::Value);
    assert_eq!(result.description.summary, summary, "{name}");
    assert!(result.issues.is_empty(), "{name}");
    result
}

fn return_data(result: &TypeResult) -> &TypeNodeData {
    let graph = result.description.expansion.as_ref().unwrap();
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    let TypeNodeData::Function { members } = &root.data else {
        panic!("the checked call carrier must retain its actual function signature: {root:?}")
    };
    let signatures = members.call_signatures.as_ref().unwrap();
    assert_eq!(signatures.len(), 1);
    let TypeRef::Ref { r#ref } = &signatures[0].return_type else {
        panic!("an established return must have its admitted graph edge")
    };
    &graph
        .nodes
        .iter()
        .find(|node| &node.id == r#ref)
        .unwrap()
        .data
}

#[test]
fn single_nongeneric_initializer_quick_types_skip_arity_arguments_and_parameters() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for name in [
            "importedFixed",
            "importedInferred",
            "importedOptional",
            "importedVoidOmitted",
            "importedVoidThis",
            "importedRequiredMissing",
            "importedTooMany",
            "importedTooManyMissing",
            "importedWrongArgument",
            "importedWrongThis",
            "importedMissingRequiredType",
        ] {
            let result = selected(&project, &fixture, name, "number");
            assert!(matches!(
                result.description.expansion.as_ref().unwrap().nodes[0].data,
                TypeNodeData::Primitive {
                    name: PrimitiveName::Number
                }
            ));
        }
        selected(&project, &fixture, "importedLiteral", "\"ready\"");
    }
}

#[test]
fn actual_inferred_body_calls_check_ordinary_targets_with_unused_optional_annotations() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for name in [
            "importedCheckedFixed",
            "importedCheckedInferred",
            "importedCheckedOptional",
            "importedCheckedVoidOmitted",
            "importedCheckedVoidThis",
        ] {
            let result = selected(&project, &fixture, name, "() => number");
            assert_eq!(
                return_data(&result),
                &TypeNodeData::Primitive {
                    name: PrimitiveName::Number
                }
            );
        }
        let result = selected(
            &project,
            &fixture,
            "importedCheckedLiteral",
            "() => \"ready\"",
        );
        assert!(
            matches!(return_data(&result), TypeNodeData::Literal { value, .. } if value == "ready")
        );
    }
}

#[test]
fn genuine_any_zero_argument_calls_remain_healthy() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        selected(&project, &fixture, "importedUntypedZero", "any");
        let result = selected(
            &project,
            &fixture,
            "importedCheckedUntypedZero",
            "() => any",
        );
        assert_eq!(return_data(&result), &TypeNodeData::Any);
    }
}

#[test]
fn checked_arity_recovery_is_invalid_without_visiting_a_missing_extra_argument() {
    let fixture = fixture();
    let own_path = fixture.path("functions.ts").canonicalize().unwrap();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for name in [
            "importedCheckedRequiredMissing",
            "importedCheckedTooManyMissing",
        ] {
            let answer = answer(&project, &fixture, name);
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::InvalidSemantic
                    && error.reason == SemanticFailureReason::OverloadRecovery.into()
                    && error.path.as_deref() == Some(own_path.as_path())),
                "{name} consumes actual overload recovery, preserving the unvisited extra argument: {answer:?}"
            );
        }
    }
}

#[test]
fn checked_parameter_and_callee_recovery_preserve_their_actual_source_failures() {
    let fixture = fixture();
    let own_path = fixture.path("functions.ts").canonicalize().unwrap();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for name in [
            "importedCheckedMissingRequiredType",
            "importedCheckedMissingCallee",
            "importedMissingCallee",
        ] {
            let answer = answer(&project, &fixture, name);
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::InvalidSemantic
                    && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
                    && error.path.as_deref() == Some(own_path.as_path())),
                "{name} must retain its consumed missing-type/callee recovery: {answer:?}"
            );
        }
    }
}

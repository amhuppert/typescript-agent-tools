//! Final contextual calls distinguish rejected relations from unvisited errors.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::project::Project;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
    TypeResult,
};

const FUNCTIONS: &str = include_str!("../fixtures/types/native-call-arguments/functions.ts");
const CONSUMER: &str = include_str!("../fixtures/types/native-call-arguments/consumer.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-call-arguments/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-call-arguments/loose.json"),
        ),
        ("functions.ts", FUNCTIONS),
        ("consumer.ts", CONSUMER),
        (
            "relations.ts",
            include_str!("../fixtures/types/native-call-arguments/relations.ts"),
        ),
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
        "{name} must establish its actual contextual argument result: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
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
        panic!("actual call carrier is a function: {root:?}")
    };
    let signatures = members.call_signatures.as_ref().unwrap();
    assert_eq!(signatures.len(), 1);
    let TypeRef::Ref { r#ref } = &signatures[0].return_type else {
        panic!("established return needs an exact graph edge")
    };
    &graph
        .nodes
        .iter()
        .find(|node| &node.id == r#ref)
        .unwrap()
        .data
}

fn invalid(
    project: &Project,
    fixture: &Fixture,
    name: &str,
    reason: SemanticFailureReason,
    expression: &str,
) {
    let answer = answer(project, fixture, name);
    let path = fixture.path("functions.ts").canonicalize().unwrap();
    let expected = position(FUNCTIONS, FUNCTIONS.find(expression).unwrap());
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error))
            if error.kind == SemanticErrorKind::InvalidSemantic
                && error.reason == reason.into()
                && error.path.as_deref() == Some(path.as_path())
                && error.position == Some(expected)),
        "{name} must preserve its exact consumed failure and schedule: {answer:?}"
    );
}

#[test]
fn scalar_contextual_arguments_establish_actual_cross_file_graph_returns() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for name in [
            "importedNumber",
            "importedUnionNumber",
            "importedUnionString",
            "importedAny",
            "importedUnknown",
            "importedOptional",
            "importedVoid",
            "importedOptionalUndefined",
        ] {
            let result = selected(&project, &fixture, name, "() => number");
            assert_eq!(
                return_data(&result),
                &TypeNodeData::Primitive {
                    name: PrimitiveName::Number
                }
            );
        }
        let string = selected(&project, &fixture, "importedString", "() => string");
        assert_eq!(
            return_data(&string),
            &TypeNodeData::Primitive {
                name: PrimitiveName::String
            }
        );
        let boolean = selected(&project, &fixture, "importedBoolean", "() => boolean");
        assert_eq!(
            return_data(&boolean),
            &TypeNodeData::Primitive {
                name: PrimitiveName::Boolean
            }
        );
        let literal = selected(&project, &fixture, "importedLiteral", "() => \"ready\"");
        assert!(
            matches!(return_data(&literal), TypeNodeData::Literal { value, .. } if value == "ready")
        );
        selected(&project, &fixture, "importedEscaped", "() => \"ready\"");
        for (name, summary, expected) in [
            ("importedBigint", "() => bigint", PrimitiveName::Bigint),
            ("importedSymbol", "() => symbol", PrimitiveName::Symbol),
        ] {
            let result = selected(&project, &fixture, name, summary);
            assert_eq!(
                return_data(&result),
                &TypeNodeData::Primitive { name: expected }
            );
        }
        for (name, summary, expected) in [
            ("importedNumericEquivalent", "() => 100", "100"),
            ("importedBigintEquivalent", "() => 10n", "10"),
            ("importedFalse", "() => false", "false"),
        ] {
            let result = selected(&project, &fixture, name, summary);
            assert!(
                matches!(return_data(&result), TypeNodeData::Literal { value, .. } if value == expected)
            );
        }
    }
}

#[test]
fn source_unions_require_each_alternative_and_literal_unions_retain_native_direction() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        selected(&project, &fixture, "importedUnionSource", "() => number");
        selected(&project, &fixture, "importedLiteralUnion", "() => string");
        invalid(
            &project,
            &fixture,
            "importedUnionRejected",
            SemanticFailureReason::OverloadRecovery,
            "stringParam(mixed)",
        );
    }
}

#[test]
fn rejected_this_and_first_argument_stop_before_the_later_missing_argument() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (name, expression) in [
            ("importedWrongPrimitive", "numberParam(\"bad\")"),
            ("importedWrongLiteral", "literalParam(\"bad\")"),
            ("importedNever", "neverParam(1)"),
            ("importedNumericRejected", "numericLiteralParam(101)"),
            ("importedBigintRejected", "bigintLiteralParam(11n)"),
            ("importedBooleanRejected", "falseLiteralParam(true)"),
            ("importedThisBeforeMissing", "withThis(Missing)"),
            ("importedWrongBeforeMissing", "twoNumbers(\"bad\", Missing)"),
        ] {
            invalid(
                &project,
                &fixture,
                name,
                SemanticFailureReason::OverloadRecovery,
                expression,
            );
        }
    }
}

#[test]
fn typed_and_untyped_calls_preserve_an_actually_visited_missing_argument() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        invalid(
            &project,
            &fixture,
            "importedMissing",
            SemanticFailureReason::UnresolvedSymbol,
            "Missing); }\nexport function /*#anyMissing*/",
        );
        invalid(
            &project,
            &fixture,
            "importedAnyMissing",
            SemanticFailureReason::UnresolvedSymbol,
            "Missing); }\n\ndeclare const mixed",
        );
    }
}

#[test]
fn null_argument_applicability_follows_effective_strict_null_checks() {
    let fixture = fixture();
    let strict = fixture.load();
    invalid(
        &strict,
        &fixture,
        "importedNull",
        SemanticFailureReason::OverloadRecovery,
        "numberParam(null)",
    );
    let loose = fixture.load_config("loose.json");
    selected(&loose, &fixture, "importedNull", "() => number");
}

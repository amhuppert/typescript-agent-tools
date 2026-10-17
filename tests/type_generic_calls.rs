//! Generic call substitution keeps candidate rejection separate from failure.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::project::Project;
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest,
};

const SITES: &str = include_str!("../fixtures/types/native-generic-calls/sites.ts");
const DEFINITIONS: &str = include_str!("../fixtures/types/native-generic-calls/definitions.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-generic-calls/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-generic-calls/loose.json"),
        ),
        ("definitions.ts", DEFINITIONS),
        ("sites.ts", SITES),
    ])
}

fn answer(project: &Project, fixture: &Fixture, name: &str) -> Result<TypeAnswer, AnalysisError> {
    let offset = SITES.find(&format!("{name} =")).unwrap();
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("sites.ts"),
        position: position(SITES, offset),
    });
    request.expand = true;
    request.depth = 2;
    project.type_of(request)
}

fn found(project: &Project, fixture: &Fixture, name: &str, expected: &str) -> TypeNodeData {
    let answer = answer(project, fixture, name);
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
        "{name} must complete its native generic call: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.description.summary, expected, "{name}");
    assert!(result.issues.is_empty(), "{name}");
    let graph = result.description.expansion.unwrap();
    graph
        .nodes
        .into_iter()
        .find(|node| node.id == graph.root)
        .unwrap()
        .data
}

fn failure_position(source: &str, marker: &str, token: &str) -> SourcePosition {
    let start = source.find(marker).unwrap();
    position(source, start + source[start..].find(token).unwrap())
}

fn failed(
    project: &Project,
    fixture: &Fixture,
    name: &str,
    reason: SemanticFailureReason,
    file: &str,
    expected: SourcePosition,
) {
    let answer = answer(project, fixture, name);
    let path = fixture.path(file).canonicalize().unwrap();
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error))
        if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == reason.into()
            && error.path.as_deref() == Some(path.as_path()) && error.position == Some(expected)),
        "{name} must preserve the actual visited failure and source: {answer:?}"
    );
}

#[test]
fn explicit_generic_arguments_establish_exact_intrinsic_literal_and_union_roots() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (name, summary, primitive) in [
            ("stringResult", "string", PrimitiveName::String),
            ("numberResult", "number", PrimitiveName::Number),
            ("booleanResult", "boolean", PrimitiveName::Boolean),
            ("constrainedResult", "string", PrimitiveName::String),
            ("unionConstraintResult", "number", PrimitiveName::Number),
            ("unknownConstraintResult", "number", PrimitiveName::Number),
        ] {
            assert_eq!(
                found(&project, &fixture, name, summary),
                TypeNodeData::Primitive { name: primitive }
            );
        }
        for name in ["literalResult", "literalConstraintResult", "relatedResult"] {
            assert_eq!(
                found(&project, &fixture, name, "\"ready\""),
                TypeNodeData::Literal {
                    base: PrimitiveName::String,
                    value: "ready".into()
                }
            );
        }
        assert!(matches!(
            found(&project, &fixture, "unionResult", "string | number"),
            TypeNodeData::Union { .. }
        ));
        assert_eq!(
            found(&project, &fixture, "nullResult", "null"),
            TypeNodeData::Null
        );
    }
}

#[test]
fn ordered_defaults_and_rejected_constraints_preserve_later_healthy_candidates() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (name, summary, primitive) in [
            ("defaultedResult", "number", PrimitiveName::Number),
            ("defaultOverrideResult", "string", PrimitiveName::String),
            ("dependentResult", "string", PrimitiveName::String),
            ("dependentOverrideResult", "number", PrimitiveName::Number),
            ("unusedDefaultResult", "number", PrimitiveName::Number),
            (
                "unusedDependentDefaultResult",
                "string",
                PrimitiveName::String,
            ),
            ("skippedConstraintResult", "number", PrimitiveName::Number),
            ("forwardOverrideResult", "string", PrimitiveName::String),
            (
                "omittedBrokenConstraintResult",
                "number",
                PrimitiveName::Number,
            ),
        ] {
            assert_eq!(
                found(&project, &fixture, name, summary),
                TypeNodeData::Primitive { name: primitive }
            );
        }
        assert_eq!(
            found(&project, &fixture, "candidateResult", "\"number\""),
            TypeNodeData::Literal {
                base: PrimitiveName::String,
                value: "number".into()
            }
        );
    }
}

#[test]
fn fully_rejected_generic_candidates_remain_invalid_without_visiting_skipped_arguments() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (name, call) in [
            ("wrongConstraintResult", "constrained<number>(1)"),
            (
                "wrongLiteralConstraintResult",
                "literalConstraint<\"other\">(\"other\")",
            ),
            ("wrongRelatedResult", "related<string, number>(1)"),
            ("wrongArgumentResult", "constrained<string>(1)"),
            (
                "rejectedConstraintBeforeMissingResult",
                "constrained<number>(MissingArgument)",
            ),
            (
                "extraMissingArgumentResult",
                "identity<number>(1, MissingArgument)",
            ),
        ] {
            failed(
                &project,
                &fixture,
                name,
                SemanticFailureReason::OverloadRecovery,
                "sites.ts",
                failure_position(SITES, &format!("{name} ="), call),
            );
        }
    }
}

#[test]
fn consumed_generic_type_argument_argument_constraint_and_default_failures_keep_their_own_sources()
{
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (name, token) in [
            ("missingTypeResult", "MissingType"),
            ("missingArgumentResult", "MissingArgument"),
        ] {
            failed(
                &project,
                &fixture,
                name,
                SemanticFailureReason::UnresolvedSymbol,
                "sites.ts",
                failure_position(SITES, &format!("{name} ="), token),
            );
        }
        failed(
            &project,
            &fixture,
            "missingConstraintResult",
            SemanticFailureReason::UnresolvedSymbol,
            "definitions.ts",
            failure_position(DEFINITIONS, "brokenConstraint<T", "MissingConstraint"),
        );
        failed(
            &project,
            &fixture,
            "missingDefaultResult",
            SemanticFailureReason::UnresolvedSymbol,
            "definitions.ts",
            failure_position(DEFINITIONS, "brokenDependent<T", "MissingDefault"),
        );
        failed(
            &project,
            &fixture,
            "defaultBeforeConstraintResult",
            SemanticFailureReason::UnresolvedSymbol,
            "definitions.ts",
            failure_position(DEFINITIONS, "defaultBeforeConstraint<T", "MissingDefault"),
        );
    }
}

#[test]
fn missing_forward_default_slot_stays_native_error_type_even_without_a_diagnostic() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        failed(
            &project,
            &fixture,
            "forwardDefaultResult",
            SemanticFailureReason::ErrorType,
            "definitions.ts",
            failure_position(DEFINITIONS, "forwardDefault<T", "V, V = string"),
        );
    }
}

//! Final reads use actual closure and canonical last-assignment questions.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-capture-flow/flow.ts");
const STRICT: &str = include_str!("../fixtures/types/native-capture-flow/tsconfig.json");
const LOOSE: &str = include_str!("../fixtures/types/native-capture-flow/loose.json");

fn check(configs: &[&str], markers: &[&str], expected: Option<&str>) {
    let fixture = Fixture::new(&[
        ("flow.ts", SOURCE),
        ("tsconfig.json", STRICT),
        ("loose.json", LOOSE),
    ]);
    for &config in configs {
        let project = fixture.load_config(config);
        for marker in markers {
            let comment = format!("/*{marker}*/ ");
            let offset = SOURCE.find(&comment).unwrap() + comment.len();
            let answer = project.type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path("flow.ts"),
                position: position(SOURCE, offset),
            }));
            if let Some(summary) = expected {
                assert!(
                    matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
                    "native captured identity must be established ({config}, {marker}): {answer:?}"
                );
                let TypeAnswer::Selected(result) = answer.unwrap() else {
                    unreachable!()
                };
                assert_eq!(result.description.summary, summary, "{config}: {marker}");
                assert!(result.issues.is_empty());
            } else {
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error))
                    if error.kind == SemanticErrorKind::UnsupportedSemantic
                    && error.reason == SemanticFailureReason::FlowRequired.into()),
                    "the actual changed or definite-assignment read consumes F ({config}, {marker}): {answer:?}"
                );
            }
        }
    }
}

#[test]
fn initialized_constant_mutable_and_parameter_capture_keep_scalar_identity() {
    check(
        &["tsconfig.json", "loose.json"],
        &[
            "constant-arrow",
            "constant-function",
            "mutable-arrow",
            "mutable-function",
            "parameter-no-assignment",
        ],
        Some("number"),
    );
}

#[test]
fn later_nested_and_exported_assignments_preserve_native_capture_boundaries() {
    check(
        &["tsconfig.json", "loose.json"],
        &[
            "before-last",
            "declaration-after-last",
            "nested-assignment",
            "constant-refined-declaration",
            "exported-let",
            "module-local-let",
            "parameter-before-last",
        ],
        Some("string | number"),
    );
}

#[test]
fn compound_assignment_positions_extend_through_the_complete_statement() {
    check(
        &["tsconfig.json", "loose.json"],
        &["compound-inside", "compound-after"],
        Some("string | number"),
    );
}

#[test]
fn only_object_literal_and_class_expression_members_extend_constant_capture() {
    check(
        &["tsconfig.json", "loose.json"],
        &[
            "class-declaration-method",
            "class-declaration-getter",
            "class-declaration-setter",
        ],
        Some("string | number"),
    );
    check(
        &["tsconfig.json", "loose.json"],
        &[
            "object-method",
            "object-getter",
            "object-setter",
            "class-expression-method",
            "class-expression-getter",
            "class-expression-setter",
        ],
        None,
    );
}

#[test]
fn known_constant_or_past_last_assignment_refinement_still_requires_flow() {
    check(
        &["tsconfig.json", "loose.json"],
        &[
            "after-last",
            "function-after-last",
            "constant-refined-arrow",
            "constant-refined-expression",
            "parameter-after-last",
            "type-only-export",
        ],
        None,
    );
}

#[test]
fn definite_assignment_anywhere_and_compound_assignment_have_distinct_initialization() {
    check(
        &["tsconfig.json", "loose.json"],
        &["definitely-assigned-before", "definitely-assigned-after"],
        Some("number"),
    );
    check(
        &["tsconfig.json"],
        &["never-initialized", "compound-not-definite"],
        None,
    );
    check(
        &["loose.json"],
        &["never-initialized", "compound-not-definite"],
        Some("number"),
    );
}

//! Final reads prove the native binder's mutation and initializer questions.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-basic-flow/flow.ts");
const STRICT: &str = include_str!("../fixtures/types/native-basic-flow/tsconfig.json");
const LOOSE: &str = include_str!("../fixtures/types/native-basic-flow/loose.json");
const GLOBALS: &str = include_str!("../fixtures/types/native-basic-flow/globals.d.ts");

fn check(markers: &[&str], expected: Option<&str>) {
    check_in(&["tsconfig.json", "loose.json"], markers, expected);
}

fn check_in(configs: &[&str], markers: &[&str], expected: Option<&str>) {
    let fixture = Fixture::new(&[
        ("flow.ts", SOURCE),
        ("globals.d.ts", GLOBALS),
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
                    matches!(&answer, Ok(TypeAnswer::Selected(_))),
                    "native identity ({config}, {marker}) must succeed: {answer:?}"
                );
                let TypeAnswer::Selected(result) = answer.unwrap() else {
                    unreachable!()
                };
                assert_eq!(result.description.summary, summary, "{config}: {marker}");
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert!(result.issues.is_empty());
            } else {
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error))
                    if error.kind == SemanticErrorKind::UnsupportedSemantic
                    && error.reason == SemanticFailureReason::FlowRequired.into()),
                    "native consumed refinement ({config}, {marker}) must stay flow_required: {answer:?}"
                );
            }
        }
    }
}

#[test]
fn default_initializers_join_real_effects_without_inventing_a_pending_mutation() {
    check(
        &["parameter-default", "pattern-default", "binding-default"],
        Some("number"),
    );
}

#[test]
fn delete_and_all_destructuring_targets_preserve_unrelated_reference_identity() {
    check(
        &[
            "delete-property",
            "delete-element",
            "delete-parentheses",
            "delete-optional",
            "array-assignment",
            "object-assignment",
            "array-default-assignment",
            "object-default-assignment",
            "nested-assignment",
            "array-rest-assignment",
            "object-rest-assignment",
            "shorthand-default-assignment",
        ],
        Some("number"),
    );
}

#[test]
fn matching_destructuring_assignment_preserves_identity_and_consumed_errors() {
    check(
        &["matching-array-assignment", "matching-spread-assignment"],
        Some("string | number"),
    );
    let fixture = Fixture::new(&[
        ("flow.ts", SOURCE),
        ("globals.d.ts", GLOBALS),
        ("tsconfig.json", STRICT),
        ("loose.json", LOOSE),
    ]);
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        let marker = "/*matching-object-assignment*/ ";
        let answer = project.type_of(TypeRequest::new(TypeQuery::At {
            file: fixture.path("flow.ts"),
            position: position(SOURCE, SOURCE.find(marker).unwrap() + marker.len()),
        }));
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error))
            if error.kind == SemanticErrorKind::InvalidSemantic
            && error.reason == SemanticFailureReason::ErrorType.into()),
            "native property destructuring consumes errorType before final admission: {config}: {answer:?}"
        );
    }
}

#[test]
fn only_native_array_mutation_forms_retain_conditional_effects() {
    let mutations = &[
        "push-effect",
        "unshift-effect",
        "escaped-push-effect",
        "element-effect",
    ];
    // The native completed join restores the actual declared identity in
    // both profiles. Intermediate narrowing is consumed by the union owner.
    check(mutations, Some("number | \"ready\""));
    check(&["pop-pure", "element-call-pure"], Some("string | number"));
}

#[test]
fn a_declared_initial_branch_stops_before_later_refinement_or_invalid_effects() {
    check(&["default-first-entry"], Some("number"));
    check(&["first-declared-branch"], Some("number | \"ready\""));
}

#[test]
fn nested_pattern_initializers_and_ordinary_assignment_patterns_have_distinct_order() {
    check(&["nested-pattern-key"], None);
    check(&["ordinary-pattern-key"], Some("number | \"ready\""));
}

//! Native flow recursion limits are separate from product work budgets.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

fn source_for(branches: usize, inside: bool, assignments: usize) -> String {
    let mut source = String::from(
        "export function flowLimit(): void {\nlet other: any;\nconst value: number = 1;\n",
    );
    source.push_str(&"if (other) {}\n".repeat(branches));
    source.push_str(&"other = 1;\n".repeat(assignments));
    source.push_str(if inside {
        "if (other) { /*#query*/value; }\n"
    } else {
        "/*#query*/value;\n"
    });
    source.push_str("}\n");
    source
}

fn answers(source: String, expected_failure: Option<SemanticErrorKind>) {
    let fixture = Fixture::new(&[
        ("flow.ts", &source),
        (
            "strict.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true},"files":["flow.ts"]}"#,
        ),
        (
            "loose.json",
            r#"{"compilerOptions":{"strict":false,"noLib":true},"files":["flow.ts"]}"#,
        ),
    ]);
    let offset = source.find("/*#query*/").unwrap() + "/*#query*/".len();
    let selected = position(&source, offset);
    for config in ["strict.json", "loose.json"] {
        let project = fixture.load_config(config);
        let answer = project.type_of(TypeRequest::new(TypeQuery::At {
            file: fixture.path("flow.ts"),
            position: selected,
        }));
        if let Some(kind) = expected_failure {
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == kind),
                "{kind:?}/{config}: {answer:?}"
            );
            let Err(AnalysisError::Semantic(error)) = answer else {
                unreachable!()
            };
            if kind == SemanticErrorKind::InvalidSemantic {
                assert_eq!(error.reason, SemanticFailureReason::ErrorType.into());
                assert_eq!(error.path, Some(fixture.path("flow.ts")));
                assert_eq!(error.position, Some(selected));
            } else {
                assert_eq!(kind, SemanticErrorKind::OutputLimit);
                assert_eq!(
                    error.reason,
                    typescript_parser::analysis::SemanticResourceReason::OutputLimit.into()
                );
                assert!(error.message.contains("cause count"));
            }
        } else {
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.outcome, TypeOutcome::Found, "{config}");
            assert_eq!(result.description.summary, "number");
            assert!(result.issues.is_empty());
        }
    }
}

#[test]
fn native_recursive_flow_depth_1999_still_returns_the_exact_read() {
    answers(linear_source(1999), None);
}

#[test]
fn native_recursive_flow_depth_at_2000_remains_an_invalid_semantic_answer() {
    answers(
        linear_source(2000),
        Some(SemanticErrorKind::InvalidSemantic),
    );
}

#[test]
fn native_recursive_flow_depth_over_2000_keeps_the_same_failure() {
    answers(
        linear_source(2001),
        Some(SemanticErrorKind::InvalidSemantic),
    );
}

#[test]
fn long_flat_assignment_flow_is_iterative_and_does_not_consume_recursive_depth() {
    answers(source_for(0, false, 4000), None);
}

fn linear_source(guards: usize) -> String {
    format!(
        "export function flowLimit(): void {{\nlet other: any;\nconst value: number = 1;\n{}/*#query*/value;\n}}\n",
        "if (other) throw 0;\n".repeat(guards)
    )
}

#[test]
fn shared_diamond_recovery_retains_the_exact_cause_count_output_limit() {
    // Native still returns errorType at depth2000. Each subsequent Shared hit
    // consumes its completion independently; the exponentially repeated causal
    // dependency list exceeds representable omittedCauses metadata. That whole
    // output failure must not be clamped or silently turned into ErrorType.
    for branches in [1000, 1001] {
        answers(
            source_for(branches, false, 0),
            Some(SemanticErrorKind::OutputLimit),
        );
    }
    answers(source_for(999, true, 0), None);
}

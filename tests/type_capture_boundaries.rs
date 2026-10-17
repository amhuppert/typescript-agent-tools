//! Final capture reads distinguish every admitted native statement boundary.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-capture-boundaries/boundaries.ts");
const STRICT: &str = include_str!("../fixtures/types/native-capture-boundaries/tsconfig.json");
const LOOSE: &str = include_str!("../fixtures/types/native-capture-boundaries/loose.json");

fn check(text: &str, bom: &str, configs: &[&str], markers: &[&str], summary: Option<&str>) {
    let disk = format!("{bom}{text}");
    let fixture = Fixture::new(&[
        ("boundaries.ts", &disk),
        ("tsconfig.json", STRICT),
        ("loose.json", LOOSE),
    ]);
    for &config in configs {
        let project = fixture.load_config(config);
        for marker in markers {
            let comment = format!("/*{marker}*/ ");
            let offset = text.find(&comment).unwrap() + comment.len();
            let answer = project.type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path("boundaries.ts"),
                position: position(text, offset),
            }));
            if let Some(summary) = summary {
                assert!(
                    matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
                    "native full identity must be established ({config}, {marker}): {answer:?}"
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
                    "actual narrowed or definite-assignment dependency must consume F ({config}, {marker}): {answer:?}"
                );
            }
        }
    }
}

#[test]
fn assignment_positions_extend_through_every_admitted_statement_ending() {
    check(
        SOURCE,
        "",
        &["tsconfig.json", "loose.json"],
        &[
            "variable-inside",
            "expression-inside",
            "if-inside",
            "do-inside",
            "while-inside",
            "for-inside",
            "for-in-inside",
            "for-of-inside",
            "switch-inside",
            "try-inside",
            "class-inside",
            "if-after",
            "while-after",
            "for-after",
            "for-in-after",
            "for-of-after",
        ],
        Some("string | number"),
    );
    check(
        SOURCE,
        "",
        &["tsconfig.json", "loose.json"],
        &[
            "variable-after",
            "expression-after",
            "do-after",
            "switch-after",
            "try-after",
            "class-after",
        ],
        None,
    );
}

#[test]
fn nested_capture_extends_each_eligible_container_and_preserves_shadow_identity() {
    check(
        SOURCE,
        "",
        &["tsconfig.json", "loose.json"],
        &["nested-declaration"],
        Some("string | number"),
    );
    check(
        SOURCE,
        "",
        &["tsconfig.json", "loose.json"],
        &["nested-arrow", "shadow-read"],
        None,
    );
}

#[test]
fn nested_assignment_sentinel_does_not_erase_definite_assignment_kind() {
    check(
        SOURCE,
        "",
        &["tsconfig.json", "loose.json"],
        &["nested-definite"],
        Some("number"),
    );
    check(SOURCE, "", &["tsconfig.json"], &["nested-compound"], None);
    check(
        SOURCE,
        "",
        &["loose.json"],
        &["nested-compound"],
        Some("number"),
    );
}

#[test]
fn raw_order_and_complete_statement_end_survive_all_line_breaks_and_leading_bom() {
    for line_break in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
        let text = SOURCE.replace('\n', line_break);
        for bom in ["", "\u{feff}"] {
            check(
                &text,
                bom,
                &["tsconfig.json", "loose.json"],
                &[
                    "variable-inside",
                    "if-inside",
                    "class-inside",
                    "nested-declaration",
                ],
                Some("string | number"),
            );
            check(
                &text,
                bom,
                &["tsconfig.json", "loose.json"],
                &["variable-after", "class-after", "shadow-read"],
                None,
            );
        }
    }
}

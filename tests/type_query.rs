//! Final public typeof type-node and actual operand-value questions.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-type-query/queries.ts");
const EXPORTS: &str = include_str!("../fixtures/types/native-type-query/exports.ts");
const BARREL: &str = include_str!("../fixtures/types/native-type-query/barrel.ts");
const GLOBALS: &str = include_str!("../fixtures/types/native-type-query/globals.ts");
const STRICT: &str = include_str!("../fixtures/types/native-type-query/tsconfig.json");
const LOOSE: &str = include_str!("../fixtures/types/native-type-query/loose.json");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("queries.ts", SOURCE),
        ("exports.ts", EXPORTS),
        ("barrel.ts", BARREL),
        ("globals.ts", GLOBALS),
        ("tsconfig.json", STRICT),
        ("loose.json", LOOSE),
    ])
}

fn request(fixture: &Fixture, marker: &str) -> TypeRequest {
    let comment = format!("/*{marker}*/");
    let end = SOURCE.find(&comment).unwrap() + comment.len();
    let offset = end + usize::from(SOURCE.as_bytes()[end] == b' ');
    TypeRequest::new(TypeQuery::At {
        file: fixture.path("queries.ts"),
        position: position(SOURCE, offset),
    })
}

#[test]
fn whole_type_nodes_and_actual_operand_values_keep_literal_object_and_alias_questions() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (prefix, summary) in [
            ("fixed", "1"),
            ("count", "number"),
            ("import", "number"),
            ("object", "{ text: string; count: number; }"),
            ("type-import", "1"),
            ("type-object", "{ text: string; count: number; }"),
            ("choice", "string | number"),
            ("specifier", "1"),
            ("specifier-object", "{ text: string; count: number; }"),
            ("exported", "1"),
            ("exported-object", "{ text: string; count: number; }"),
        ] {
            for (suffix, question) in [("type", TypeQuestion::Type), ("value", TypeQuestion::Value)]
            {
                let marker = format!("{prefix}-{suffix}");
                let answer = project.type_of(request(&fixture, &marker));
                assert!(
                    matches!(&answer, Ok(TypeAnswer::Selected(_))),
                    "the exact {config} {marker} question must establish: {answer:?}"
                );
                let TypeAnswer::Selected(result) = answer.unwrap() else {
                    unreachable!()
                };
                assert_eq!(result.target.kind, question, "{marker}");
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(result.description.summary, summary, "{marker}");
                assert!(result.issues.is_empty());
            }
        }
    }
}

#[test]
fn type_query_initialization_suppression_is_narrow_and_preserves_flow_obligations() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (suffix, question) in [("type", TypeQuestion::Type), ("value", TypeQuestion::Value)] {
            let marker = format!("pending-{suffix}");
            let answer = project.type_of(request(&fixture, &marker));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "typeof suppresses only nonautomatic initialization checking: {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.target.kind, question);
            assert_eq!(result.description.summary, "number");
            assert_eq!(result.outcome, TypeOutcome::Found);
            for prefix in ["automatic", "narrowed"] {
                let marker = format!("{prefix}-{suffix}");
                let answer = project.type_of(request(&fixture, &marker));
                if prefix == "automatic" && config == "loose.json" {
                    assert!(
                        matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.description.summary == "any"),
                        "{marker}: {answer:?}"
                    );
                } else {
                    assert!(
                        matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::UnsupportedSemantic && error.reason == SemanticFailureReason::FlowRequired.into()),
                        "typeof must preserve the admitted flow boundary {config}/{marker}: {answer:?}"
                    );
                }
            }
        }
        let ordinary = project.type_of(request(&fixture, "ordinary-pending"));
        if config == "tsconfig.json" {
            assert!(
                matches!(&ordinary, Err(AnalysisError::Semantic(error)) if error.reason == SemanticFailureReason::FlowRequired.into()),
                "ordinary definite assignment remains excluded: {ordinary:?}"
            );
        } else {
            assert!(
                matches!(&ordinary, Ok(TypeAnswer::Selected(result)) if result.description.summary == "number"),
                "{ordinary:?}"
            );
        }
    }
}

#[test]
fn consumed_query_recovery_stays_invalid_and_supported_query_forms_keep_native_types() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for suffix in ["type", "value"] {
            let marker = format!("missing-{suffix}");
            let answer = project.type_of(request(&fixture, &marker));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::ErrorType.into()),
                "the consumed unresolved operand is native error recovery: {answer:?}"
            );
            let marker = format!("qualified-{suffix}");
            let answer = project.type_of(request(&fixture, &marker));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.description.summary == "number" && result.issues.is_empty()),
                "qualified property query preserves the native type: {answer:?}"
            );
            let marker = format!("this-{suffix}");
            let answer = project.type_of(request(&fixture, &marker));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.description.summary == "this" && result.issues.is_empty()),
                "this query preserves the native type: {answer:?}"
            );
        }
        let answer = project.type_of(request(&fixture, "instantiated-type"));
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.description.summary == "typeof generic<string>" && result.issues.is_empty()),
            "TypeQuery type arguments instantiate the signature: {answer:?}"
        );
        let answer = project.type_of(request(&fixture, "instantiated-value"));
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.target.kind == TypeQuestion::Value && result.description.summary == "<T>(value: T) => T"),
            "the operand value has its separate non-instantiated question: {answer:?}"
        );
    }
}

#[test]
fn runtime_type_only_access_and_type_argument_context_remain_distinct() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [
            ("runtime-type-import", "1"),
            ("runtime-type-object", "{ text: string; count: number; }"),
            ("runtime-specifier", "1"),
            (
                "runtime-specifier-object",
                "{ text: string; count: number; }",
            ),
            ("runtime-exported", "1"),
            (
                "runtime-exported-object",
                "{ text: string; count: number; }",
            ),
        ] {
            let answer = project.type_of(request(&fixture, marker));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.target.kind == TypeQuestion::Expression && result.outcome == TypeOutcome::Found && result.description.summary == summary),
                "the native type-only alias diagnostic does not replace its healthy type: {answer:?}"
            );
        }
        let answer = project.type_of(request(&fixture, "argument-type"));
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
            "TypeQuery type-argument identifiers stay ordinary type questions: {answer:?}"
        );
        let comment = SOURCE.find("/*fixed-value*/").unwrap() + 3;
        let answer = project.type_of(TypeRequest::new(TypeQuery::At {
            file: fixture.path("queries.ts"),
            position: position(SOURCE, comment),
        }));
        assert!(
            matches!(answer, Ok(TypeAnswer::Unselected(_))),
            "query trivia must remain unselected: {answer:?}"
        );
    }
}

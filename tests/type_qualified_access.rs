//! Qualified typeof operands retain each selected segment and native flow.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/qualified-access/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/qualified-access/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/qualified-access/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/qualified-access/loose.json"),
        ),
        (
            "unchecked.json",
            include_str!("../fixtures/types/qualified-access/unchecked.json"),
        ),
    ])
}

fn request(fixture: &Fixture, name: &str, expand: bool) -> TypeRequest {
    let marker = format!("/*#{name}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len()),
    });
    request.expand = expand;
    request
}

#[test]
fn qualified_queries_keep_receiver_segment_and_type_node_questions() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "unchecked.json"] {
        let project = fixture.load_config(config);
        let optional = if config == "loose.json" {
            "string"
        } else {
            "string | undefined"
        };
        for (name, kind, summary) in [
            ("whole", TypeQuestion::Type, "number"),
            ("root", TypeQuestion::Value, "Holder"),
            ("middle", TypeQuestion::Value, "Inner"),
            ("leaf", TypeQuestion::Value, "number"),
            ("fixed", TypeQuestion::Type, "42"),
            ("optional", TypeQuestion::Type, optional),
            (
                "indexed",
                TypeQuestion::Type,
                if config == "unchecked.json" {
                    "number | undefined"
                } else {
                    "number"
                },
            ),
            ("any", TypeQuestion::Type, "any"),
            ("nullable", TypeQuestion::Type, "42"),
            ("unrelated", TypeQuestion::Type, optional),
            ("loop", TypeQuestion::Type, optional),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, name, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}: {answer:?}");
                };
                assert_eq!(result.target.kind, kind, "{config}/{name}");
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{name}");
                assert_eq!(result.description.summary, summary, "{config}/{name}");
            }
        }
    }
}

#[test]
fn qualified_optional_assignment_requires_flow_and_missing_property_is_invalid() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "unchecked.json"] {
        let project = fixture.load_config(config);
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, "assigned", expand));
            if config == "loose.json" {
                assert!(
                    matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.description.summary == "string"),
                    "{answer:?}"
                );
            } else {
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::UnsupportedSemantic && error.reason == SemanticFailureReason::FlowRequired.into()),
                    "{answer:?}"
                );
            }
            let answer = project.type_of(request(&fixture, "missing", expand));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::ErrorType.into()),
                "{answer:?}"
            );
        }
    }
}

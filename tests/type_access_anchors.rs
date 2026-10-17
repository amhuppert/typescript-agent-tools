//! Access anchors preserve the exact native operation and its flow admission.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/access-anchors/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/access-anchors/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/access-anchors/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/access-anchors/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/access-anchors/exact.json"),
        ),
    ])
}

#[test]
fn access_name_and_closing_tokens_select_native_read_or_write_questions() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (name, summary) in [
            ("read", "number"),
            ("readAccessor", "string"),
            ("writeAccessor", "boolean"),
            ("compoundAccessor", "string"),
            (
                "writeOptional",
                if config == "tsconfig.json" {
                    "string | undefined"
                } else {
                    "string"
                },
            ),
            ("readonlyIndex", "number"),
            ("element", "number"),
            ("key", "\"value\""),
            ("call", "number"),
        ] {
            let marker = format!("/*#{name}*/");
            let offset = SOURCE.find(&marker).unwrap() + marker.len();
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, offset),
                });
                request.expand = expand;
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{name}");
                assert_eq!(
                    result.target.kind,
                    TypeQuestion::Expression,
                    "{config}/{name}"
                );
                assert_eq!(result.description.summary, summary, "{config}/{name}");
            }
        }
    }
}

#[test]
fn missing_and_readonly_property_accesses_consume_error_types() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for name in ["missing", "readonlyProperty"] {
            let marker = format!("/*#{name}*/");
            let request = TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len()),
            });
            let answer = project.type_of(request);
            assert!(
                matches!(answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::ErrorType.into()),
                "{config}/{name} must consume native errorType"
            );
        }
    }
}

#[test]
fn access_flow_distinguishes_unrelated_assignment_from_optional_refinement() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for name in ["unrelatedRead", "assignedRead"] {
            let request = TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(SOURCE, SOURCE.find(&format!("const {name}")).unwrap() + 6),
            });
            let answer = project.type_of(request);
            if name == "assignedRead" && config != "loose.json" {
                assert!(
                    matches!(answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::UnsupportedSemantic
                    && error.reason == SemanticFailureReason::FlowRequired.into()),
                    "optional assignment refinement requires F"
                );
            } else {
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}: {answer:?}");
                };
                assert_eq!(
                    result.description.summary,
                    if config == "loose.json" {
                        "string"
                    } else {
                        "string | undefined"
                    },
                    "{config}/{name}"
                );
            }
        }
    }
}

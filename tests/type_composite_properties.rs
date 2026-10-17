//! Actual union/intersection symbols feed final access and flow queries.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-composite-properties/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-composite-properties/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-composite-properties/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-composite-properties/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-composite-properties/exact.json"),
        ),
    ])
}
#[test]
fn composite_reads_and_writes_preserve_native_property_values() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [
            ("optionalConflict", "number"),
            ("originalNever", "number"),
            ("wideConflict", "number"),
            ("union", "string | number"),
            ("intersection", "never"),
            ("same", "number"),
            (
                "optional",
                if config == "loose.json" {
                    "number"
                } else {
                    "number | undefined"
                },
            ),
            ("required", "number"),
            ("intersectionWrite", "number"),
            ("accessorRead", "string | number"),
            ("accessorWrite", "string | number | boolean"),
            ("indexed", "number"),
            ("deferred", "string | number | boolean"),
            (
                "restored",
                if config == "loose.json" {
                    "A"
                } else {
                    "A | undefined"
                },
            ),
            ("joined", "{ p: 0; a: number; } | { p: 1; b: number; }"),
        ] {
            for expand in [false, true] {
                let tag = format!("/*#{marker}*/");
                let start = SOURCE.find(&tag).unwrap() + tag.len();
                let offset = if ["restored", "joined"].contains(&marker) {
                    start
                } else {
                    start
                        + SOURCE[start..]
                            .find(
                                if ["optionalConflict", "originalNever", "wideConflict"]
                                    .contains(&marker)
                                {
                                    ".q"
                                } else {
                                    ".p"
                                },
                            )
                            .unwrap()
                        + 1
                };
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, offset),
                });
                request.expand = expand;
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{marker}");
                assert_eq!(result.description.summary, summary, "{config}/{marker}");
                assert!(result.issues.is_empty(), "{config}/{marker}");
            }
        }
    }
}
#[test]
fn missing_readonly_and_consumed_invalid_members_keep_distinct_failures() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, kind, reason) in [
            (
                "literalConflict",
                SemanticErrorKind::InvalidSemantic,
                SemanticFailureReason::ErrorType,
            ),
            (
                "privateConflict",
                SemanticErrorKind::InvalidSemantic,
                SemanticFailureReason::ErrorType,
            ),
            (
                "partial",
                SemanticErrorKind::InvalidSemantic,
                SemanticFailureReason::ErrorType,
            ),
            (
                "readonlyWrite",
                SemanticErrorKind::InvalidSemantic,
                SemanticFailureReason::ErrorType,
            ),
            (
                "unhealthy",
                SemanticErrorKind::InvalidSemantic,
                SemanticFailureReason::UnresolvedSymbol,
            ),
            (
                "narrowed",
                SemanticErrorKind::UnsupportedSemantic,
                SemanticFailureReason::FlowRequired,
            ),
        ] {
            for expand in [false, true] {
                let tag = format!("/*#{marker}*/");
                let start = SOURCE.find(&tag).unwrap() + tag.len();
                let property = if marker == "partial" {
                    ".a"
                } else if ["literalConflict", "privateConflict"].contains(&marker) {
                    ".q"
                } else {
                    ".p"
                };
                let offset = if marker == "narrowed" {
                    start
                } else {
                    start + SOURCE[start..].find(property).unwrap() + 1
                };
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, offset),
                });
                request.expand = expand;
                let answer = project.type_of(request);
                assert!(
                    matches!(&answer,Err(AnalysisError::Semantic(error)) if error.kind==kind && error.reason==reason.into()),
                    "{config}/{marker}: {answer:?}"
                );
            }
        }
    }
}

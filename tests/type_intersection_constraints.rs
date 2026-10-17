//! Constraint substitution at actual reference-use positions.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-intersection-constraints/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-intersection-constraints/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-intersection-constraints/loose.json"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
    });
    request.expand = expand;
    request.depth = 2;
    request
}

#[test]
fn actual_reference_substitution_keeps_changed_absent_and_unchanged_constraints_distinct() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [
            ("ordinary", "{ p: string; } & { q: number; }"),
            (
                "changed",
                "({ p: string; } | { p: number; }) & { q: boolean; }",
            ),
            ("absent", "{ p: string; } | { p: number; }"),
            (
                "nested",
                "({ q: string; } | { q: number; }) & { p: boolean; }",
            ),
            (
                "nullable",
                if config == "tsconfig.json" {
                    "{ p: string; } & { q: number; }"
                } else {
                    "T & { q: number; }"
                },
            ),
            ("unchanged", "T & U"),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}: {answer:?}")
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{marker}");
                assert_eq!(result.description.summary, summary, "{config}/{marker}");
                assert!(result.issues.is_empty(), "{config}/{marker}");
            }
        }
    }
}

#[test]
fn a_later_consumed_intersection_constraint_keeps_its_original_failure() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, "invalid", expand));
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{config}: {answer:?}")
            };
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
            assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
            assert_eq!(error.path, Some(fixture.path("cases.ts")));
            assert_eq!(
                error.position,
                Some(position(SOURCE, SOURCE.find("MissingConstraint").unwrap()))
            );
        }
    }
}

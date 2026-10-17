//! Composite inventories feed final calls and index reads through shared owners.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-composite-signatures/cases.ts");
fn check(marker: &str, expected: &str) {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "unchecked.json"] {
        let project = fixture.load_config(config);
        let expected = match (marker, config) {
            ("unionIndex", "unchecked.json") => "string | number | undefined",
            ("intersectionIndex", "unchecked.json") => "undefined",
            _ => expected,
        };
        for expand in [false, true] {
            let mut request = request(&fixture, marker);
            request.expand = expand;
            let answer = project.type_of(request);
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{config}/{marker}: {answer:?}");
            };
            assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{marker}");
            assert_eq!(result.description.summary, expected, "{config}/{marker}");
            assert!(
                result.issues.is_empty(),
                "{config}/{marker}: {:?}",
                result.issues
            );
        }
    }
}
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-composite-signatures/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-composite-signatures/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-composite-signatures/loose.json"),
        ),
        (
            "unchecked.json",
            include_str!("../fixtures/types/native-composite-signatures/unchecked.json"),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    let mut offset = SOURCE.find(&tag).unwrap() + tag.len();
    if marker == "readonlyIndex" {
        offset += SOURCE[offset..].find(']').unwrap();
    }
    TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, offset),
    })
}
#[test]
fn matching_zero_argument_returns_are_unioned() {
    check("zero", "string | number");
}
#[test]
fn parameter_types_are_intersected_for_union_calls() {
    check("compatible", "1 | 2");
}
#[test]
fn optional_excess_parameters_match_partially() {
    check("partial", "1 | 2");
}
#[test]
fn incompatible_union_parameters_require_never() {
    check("disjoint", "1 | 2");
}
#[test]
fn intersection_signatures_preserve_overload_order() {
    check("intersection", "1");
}
#[test]
fn matching_generics_use_actual_parameter_identity() {
    check("identical", "1");
}
#[test]
fn combined_generics_preserve_first_defaults() {
    check("defaults", "string");
}
#[test]
fn one_overloaded_union_constituent_combines_each_candidate() {
    check("overloaded", "1 | 3");
}
#[test]
fn explicit_this_types_combine_before_applicability() {
    check("this", "1 | 2");
}
#[test]
fn union_rest_parameters_use_actual_array_elements() {
    check("rest", "1 | 2");
}
#[test]
fn union_indexes_preserve_unchecked_reads() {
    check("unionIndex", "string | number");
}
#[test]
fn intersection_indexes_reduce_values() {
    check("intersectionIndex", "never");
}
#[test]
fn readonly_index_diagnostic_preserves_native_write_type() {
    check("readonlyIndex", "number");
}
#[test]
fn consumed_composite_return_recovery_keeps_failure_reason() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "unchecked.json"] {
        let project = fixture.load_config(config);
        for (marker, reason) in [
            ("unknown", SemanticFailureReason::ErrorType),
            ("badReturn", SemanticFailureReason::UnresolvedSymbol),
        ] {
            let answer = project.type_of(request(&fixture, marker));
            assert!(
                matches!(&answer,Err(AnalysisError::Semantic(error)) if error.kind==SemanticErrorKind::InvalidSemantic && error.reason==reason.into()),
                "{config}/{marker}: {answer:?}"
            );
        }
    }
}

//! Final overload and array-call witnesses for native composite inventories.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-composite-boundaries/cases.ts");

fn check(marker: &str, expected: &str) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-composite-boundaries/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-composite-boundaries/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-composite-boundaries/loose.json"),
        ),
    ]);
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for expand in [false, true] {
            let tag = format!("/*#{marker}*/");
            let mut request = TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
            });
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

#[test]
fn union_predicates_combine_types() {
    check("predicate", "\"chosen\"");
}
#[test]
fn false_return_constituent_preserves_overload_policy() {
    check("falsePredicate", "\"fallback\"");
}
#[test]
fn ordinary_boolean_return_does_not_create_predicate() {
    check("booleanPredicate", "\"fallback\"");
}
#[test]
fn assertions_do_not_combine_as_is_predicates() {
    check("assertionPredicate", "\"fallback\"");
}
#[test]
fn predicate_parameter_positions_must_match() {
    check("mismatchPredicate", "\"fallback\"");
}
#[test]
fn mixin_returns_are_intersected_with_ordinary_constructor() {
    check("mixin", "\"chosen\"");
}
#[test]
fn all_mixin_intersections_keep_first_constructor() {
    check("allMixins", "\"chosen\"");
}
#[test]
fn readonly_any_array_is_a_mixin_rest_type() {
    check("readonlyMixin", "\"chosen\"");
}
#[test]
fn mutable_array_member_union_reuses_global_array() {
    check("mutableArrays", "(string | number)[]");
}
#[test]
fn readonly_array_member_union_preserves_readonly() {
    check("readonlyArrays", "readonly (string | number)[]");
}
#[test]
fn mixed_array_member_union_uses_readonly_fallback() {
    check("mixedArrays", "readonly (string | number)[]");
}

//! Array literal inference and const/contextual tuple construction at public queries.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-array-expressions/cases.ts");
fn check(cases: &[(&str, &str, &str, &str)]) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-array-expressions/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-array-expressions/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-array-expressions/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-array-expressions/exact.json"),
        ),
    ]);
    for (profile, config) in ["tsconfig.json", "loose.json", "exact.json"]
        .iter()
        .enumerate()
    {
        let project = fixture.load_config(config);
        for &(name, strict, loose, exact) in cases {
            let tag = format!("/*#{name}*/");
            for expand in [false, true] {
                let request = TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
                    },
                    expand,
                    depth: 2,
                };
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{name}: {:?}",
                    result.issues
                );
                assert_eq!(
                    result.description.summary,
                    [strict, loose, exact][profile],
                    "{config}/{name}"
                );
                assert!(
                    result.issues.is_empty(),
                    "{config}/{name}: {:?}",
                    result.issues
                );
            }
        }
    }
}
#[test]
fn mutable_arrays_and_spreads_reduce_actual_element_types() {
    check(&[
        ("numbers", "number[]", "number[]", "number[]"),
        (
            "mixed",
            "(string | number)[]",
            "(string | number)[]",
            "(string | number)[]",
        ),
        (
            "spread",
            "(string | number)[]",
            "(string | number)[]",
            "(string | number)[]",
        ),
        ("assertion", "string[]", "string[]", "string[]"),
    ]);
}
#[test]
fn contextual_spreads_preserve_first_last_and_empty_positions() {
    check(&[
        (
            "spreadFirst",
            "[\"x\", 2, true]",
            "[\"x\", 2, true]",
            "[\"x\", 2, true]",
        ),
        (
            "spreadLast",
            "[true, \"x\", 2]",
            "[true, \"x\", 2]",
            "[true, \"x\", 2]",
        ),
        (
            "spreadMiddle",
            "[true, \"x\", 2, false]",
            "[true, \"x\", 2, false]",
            "[true, \"x\", 2, false]",
        ),
        (
            "spreadMultiple",
            "[\"x\", 2, true, \"x\", 2]",
            "[\"x\", 2, true, \"x\", 2]",
            "[\"x\", 2, true, \"x\", 2]",
        ),
        ("spreadEmpty", "[true]", "[true]", "[true]"),
    ]);
}
#[test]
fn const_arrays_preserve_nested_tuples_and_spread_labels() {
    check(&[
        (
            "frozen",
            "readonly [1, \"two\"]",
            "readonly [1, \"two\"]",
            "readonly [1, \"two\"]",
        ),
        (
            "nested",
            "readonly [readonly [1], readonly [\"two\"]]",
            "readonly [readonly [1], readonly [\"two\"]]",
            "readonly [readonly [1], readonly [\"two\"]]",
        ),
        (
            "tupleSpread",
            "readonly [\"x\", 2]",
            "readonly [\"x\", 2]",
            "readonly [\"x\", 2]",
        ),
    ]);
}
#[test]
fn holes_distinguish_widening_undefined_from_exact_missing() {
    check(&[
        (
            "hole",
            "readonly [undefined, 1]",
            "readonly [any, 1]",
            "readonly [never?, 1?]",
        ),
        (
            "allHole",
            "readonly [undefined]",
            "readonly [any]",
            "readonly [never?]",
        ),
    ]);
}
#[test]
fn contextual_arrays_preserve_literal_categories_and_mutability() {
    check(&[
        ("contextTuple", "[1]", "[1]", "[1]"),
        (
            "contextHoles",
            "[undefined, string]",
            "[any, string]",
            "[never?, string?]",
        ),
        (
            "contextArray",
            "(\"two\" | 1)[]",
            "(\"two\" | 1)[]",
            "(\"two\" | 1)[]",
        ),
        (
            "mutableConst",
            "[1, \"two\"]",
            "[1, \"two\"]",
            "[1, \"two\"]",
        ),
    ]);
}
#[test]
fn empty_variable_arrays_preserve_the_native_automatic_marker_question() {
    check(&[
        ("empty", "any[]", "any[]", "any[]"),
        ("parenthesizedEmpty", "never[]", "any[]", "never[]"),
    ]);
}

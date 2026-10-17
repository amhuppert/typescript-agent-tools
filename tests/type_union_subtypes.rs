//! Native subtype-reduced unions through final public queries.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-union-subtypes/cases.ts");
fn check(cases: &[(&str, &str)]) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-union-subtypes/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-union-subtypes/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-union-subtypes/loose.json"),
        ),
    ]);
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for &(name, expected) in cases {
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
                    panic!("{config}/{name}/expand={expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{name}: {:?}",
                    result.issues
                );
                assert_eq!(result.description.summary, expected, "{config}/{name}");
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
fn structural_subtypes_reduce_independently_of_return_order() {
    check(&[
        ("base", "Base"),
        ("reverse", "Base"),
        ("empty", "Empty"),
        ("primitive", "\"x\" | 1"),
    ]);
}

#[test]
fn discriminants_and_union_constraints_preserve_native_constituents() {
    check(&[
        ("discriminated", "Alpha | Beta"),
        ("constrained", "Alpha | Beta"),
    ]);
}

#[test]
fn class_hierarchy_is_distinct_from_structural_compatibility() {
    check(&[("hierarchy", "Parent"), ("unrelated", "Parent | Sibling")]);
}

#[test]
fn deferred_tuple_aliases_order_by_actual_alias_identity() {
    check(&[("aliases", "A | Z")]);
}

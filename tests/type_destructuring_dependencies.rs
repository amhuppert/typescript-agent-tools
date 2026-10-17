//! Final assignment producers retain their actual dependency order.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-destructuring/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-destructuring/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-destructuring/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-destructuring/loose.json"),
        ),
        (
            "unchecked.json",
            include_str!("../fixtures/types/native-destructuring/unchecked.json"),
        ),
    ])
}

fn check(configs: &[&str], markers: &[&str]) {
    let fixture = fixture();
    for &config in configs {
        let project = fixture.load_config(config);
        for &marker in markers {
            let tag = format!("/*#{marker}*/");
            let offset = SOURCE.find(&tag).unwrap() + tag.len();
            for (expand, depth) in [(false, 0), (true, 0), (true, 2)] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, offset),
                });
                request.expand = expand;
                request.depth = depth;
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!(
                        "actual destructured producer must retain declared identity ({config}/{marker}/{depth}): {answer:?}"
                    );
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(result.description.summary, "string | number");
                assert!(result.issues.is_empty());
                if let Some(graph) = result.description.expansion {
                    let root = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == graph.root)
                        .unwrap();
                    assert!(matches!(root.data, TypeNodeData::Union { .. }));
                    if depth > 0 {
                        let TypeNodeData::Union {
                            alternatives,
                            omitted_alternatives,
                        } = &root.data
                        else {
                            unreachable!()
                        };
                        assert_eq!(alternatives.len(), 2);
                        assert_eq!(*omitted_alternatives, None);
                        assert!(!graph.truncated);
                    }
                }
            }
        }
    }
}

#[test]
fn own_property_and_shorthand_reads_leave_an_unvisited_sibling_unvisited() {
    check(
        &["tsconfig.json", "loose.json", "unchecked.json"],
        &["ownProperty", "shorthand"],
    );
}

#[test]
fn numeric_cooked_and_computed_names_use_the_actual_property_key() {
    check(
        &["tsconfig.json", "loose.json", "unchecked.json"],
        &["numericName", "exactName", "computedName"],
    );
}

#[test]
fn a_property_default_removes_undefined_before_joining_the_actual_default() {
    check(
        &["tsconfig.json", "loose.json", "unchecked.json"],
        &["propertyDefault"],
    );
}

#[test]
fn typed_array_elements_and_rest_consume_the_real_index_and_array_targets() {
    check(
        &["tsconfig.json", "loose.json", "unchecked.json"],
        &["arrayElement", "arraySpread"],
    );
}

#[test]
fn array_defaults_preserve_omitted_positions_and_unchecked_index_policy() {
    check(
        &["tsconfig.json", "loose.json", "unchecked.json"],
        &["arrayDefault", "omittedSlot"],
    );
}

#[test]
fn a_for_of_assignment_checks_its_actual_rhs_before_read_admission() {
    check(
        &["tsconfig.json", "loose.json", "unchecked.json"],
        &["forOf"],
    );
}

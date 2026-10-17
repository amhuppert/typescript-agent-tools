//! Final native variable declarations share pattern, context and widening owners.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-variable-patterns/cases.ts");
fn check(markers: &[&str]) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-variable-patterns/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-variable-patterns/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-variable-patterns/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-variable-patterns/exact.json"),
        ),
        (
            "unchecked.json",
            include_str!("../fixtures/types/native-variable-patterns/unchecked.json"),
        ),
    ]);
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-variable-patterns/expected.json"
    ))
    .unwrap();
    for config in ["tsconfig", "loose", "exact", "unchecked"] {
        let project = fixture.load_config(&format!("{config}.json"));
        for row in rows.iter().filter(|row| {
            row["config"] == config && markers.contains(&row["name"].as_str().unwrap())
        }) {
            let name = row["name"].as_str().unwrap();
            let marker = format!("/*#{name}*/");
            for expand in [false, true] {
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len()),
                    },
                    expand,
                    depth: 1,
                });
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{name}/{expand}: {:?}",
                    result.issues
                );
                assert_eq!(
                    result.description.summary,
                    row["summary"].as_str().unwrap(),
                    "{config}/{name}/{expand}"
                );
                assert!(
                    result.issues.is_empty(),
                    "{config}/{name}/{expand}: {:?}",
                    result.issues
                );
            }
        }
    }
}
#[test]
fn nested_and_renamed_object_bindings_share_actual_parent_projection() {
    check(&["object", "renamed", "nested", "anyParent"]);
}
#[test]
fn defaults_retain_annotation_and_nullability_precedence() {
    check(&[
        "defaultInferred",
        "defaultAnnotated",
        "nestedDefault",
        "optionalAnnotation",
    ]);
}
#[test]
fn array_omissions_and_rest_use_real_tuple_arguments_and_iteration() {
    check(&["arrayHead", "arrayOptional", "arrayRest"]);
}
#[test]
fn catch_loop_parameter_and_nil_class_flow_keep_distinct_questions() {
    check(&[
        "caught",
        "forIn",
        "forOf",
        "classNil",
        "parameter",
        "restParameter",
    ]);
}

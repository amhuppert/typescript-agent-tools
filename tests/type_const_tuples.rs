//! Const-variable recognition follows variadic tuple elements at native depth.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-const-tuples/cases.ts");
#[test]
fn const_variadic_tuple_contexts_preserve_literals_through_calls_and_callbacks() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-const-tuples/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-const-tuples/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-const-tuples/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-const-tuples/exact.json"),
        ),
    ]);
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/types/native-const-tuples/expected.json"
    ))
    .unwrap();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in expected.as_object().unwrap() {
            let tag = format!("/*#{marker}*/");
            for expand in [false, true] {
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
                    },
                    expand,
                    depth: 0,
                });
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{marker}");
                assert_eq!(
                    result.description.summary,
                    summary.as_str().unwrap(),
                    "{config}/{marker}"
                );
                assert!(
                    result.issues.is_empty(),
                    "{config}/{marker}: {:?}",
                    result.issues
                );
            }
        }
    }
}

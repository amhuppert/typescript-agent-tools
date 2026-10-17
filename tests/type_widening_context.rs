//! Fresh object siblings, nested contexts and exact-optional missing types.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn native_widening_contexts_preserve_nested_siblings_and_property_order() {
    let source = include_str!("../fixtures/types/native-widening-context/cases.ts");
    let fixture = Fixture::new(&[
        ("cases.ts", source),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-widening-context/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-widening-context/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-widening-context/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-widening-context/exact.json"),
        ),
    ]);
    let expected: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-widening-context/expected.json"
    ))
    .unwrap();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("{config}.json"));
        for row in expected.iter().filter(|row| row["config"] == config) {
            let name = row["name"].as_str().unwrap();
            let marker = format!("/*#{name}*/");
            for expand in [false, true] {
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(source, source.find(&marker).unwrap() + marker.len()),
                    },
                    expand,
                    depth: 1,
                });
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}/{expand}: {answer:?}")
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

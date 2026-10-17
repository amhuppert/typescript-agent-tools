//! Native recursive mapped-key relations choose the final overload return.
//! This is a candidate counterfactual for the target-Index exact-True gate;
//! passing it alone does not prove that the recursive Maybe branch occurred.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn deferred_recursive_mapped_indexes_preserve_overload_fallthrough() {
    let source = include_str!("../fixtures/types/native-mapped-deferred-index/cases.ts");
    let fixture = Fixture::new(&[
        (
            "native-variable-patterns/globals.d.ts",
            include_str!("../fixtures/types/native-variable-patterns/globals.d.ts"),
        ),
        (
            "native-mapped-deferred-index/tsconfig.json",
            include_str!("../fixtures/types/native-mapped-deferred-index/tsconfig.json"),
        ),
        ("native-mapped-deferred-index/cases.ts", source),
    ]);
    let project = fixture.load_config("native-mapped-deferred-index/tsconfig.json");
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-mapped-deferred-index/expected.json"
    ))
    .unwrap();
    for row in rows {
        let name = row["name"].as_str().unwrap();
        let marker = format!("/*#{name}*/");
        for expand in [false, true] {
            let answer = project.type_of(TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path("native-mapped-deferred-index/cases.ts"),
                    position: position(source, source.find(&marker).unwrap() + marker.len()),
                },
                expand,
                depth: 1,
            });
            let Ok(TypeAnswer::Selected(result)) = &answer else {
                panic!("{name}/{expand}: {answer:?}")
            };
            assert_eq!(
                result.outcome,
                TypeOutcome::Found,
                "{name}/{expand}: {answer:?}"
            );
            assert_eq!(
                result.description.summary,
                row["summary"].as_str().unwrap(),
                "{name}/{expand}"
            );
            assert!(
                result.issues.is_empty(),
                "{name}/{expand}: {:?}",
                result.issues
            );
        }
    }
}

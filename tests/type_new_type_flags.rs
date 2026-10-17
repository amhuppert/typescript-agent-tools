//! Fresh generic object factories must not inherit another type's resolved fields.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-new-type-flags/cases.ts");

#[test]
fn fresh_generic_anonymous_objects_keep_actual_member_results() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-new-type-flags/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-new-type-flags/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-new-type-flags/loose.json"),
        ),
    ]);
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for marker in [
            "nested",
            "direct",
            "constrained",
            "method",
            "noInferKeys",
            "emptyKeys",
            "runnerKeys",
            "mappedKeys",
        ] {
            let tag = format!("/*#{marker}*/");
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
                });
                request.expand = expand;
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{marker}: {result:?}"
                );
                let expected = match marker {
                    "noInferKeys" | "mappedKeys" => "\"value\"",
                    "emptyKeys" => "never",
                    "runnerKeys" => "\"run\"",
                    _ => "string",
                };
                assert_eq!(result.description.summary, expected, "{config}/{marker}");
                assert!(
                    result.issues.is_empty(),
                    "{config}/{marker}: {:?}",
                    result.issues
                );
            }
        }
    }
}

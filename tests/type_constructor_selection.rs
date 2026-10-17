mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn bodyless_constructors_do_not_request_property_flow() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-constructor-selection"),
    );
    for config in ["strict.json", "implicit-any.json", "nocheck.json"] {
        let project = fixture.load_config(config);
        for (file, source) in [
            (
                "cases.ts",
                include_str!("../fixtures/types/native-constructor-selection/cases.ts"),
            ),
            (
                "declarations.d.ts",
                include_str!("../fixtures/types/native-constructor-selection/declarations.d.ts"),
            ),
        ] {
            for (offset, _) in source.match_indices("/*#") {
                let end = offset + source[offset..].find("*/").unwrap() + 2;
                let marker = &source[offset + 3..end - 2];
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path(file),
                            position: position(source, end),
                        },
                        expand,
                        depth: 1,
                    });
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{config}/{file}/{marker}/{expand}: {answer:?}")
                    };
                    assert_eq!(
                        result.outcome,
                        TypeOutcome::Found,
                        "{config}/{marker}/{expand}: {result:?}"
                    );
                    assert!(result.issues.is_empty());
                    assert_eq!(
                        result.description.summary,
                        if marker == "implementedRead" {
                            "number"
                        } else {
                            "any"
                        }
                    );
                }
            }
        }
    }
}

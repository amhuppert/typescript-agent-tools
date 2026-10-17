mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-recursive-format/cases.ts");
const VARIANTS: &str = include_str!("../fixtures/types/native-recursive-format/variants.ts");
#[test]
fn recursive_format_aliases_do_not_consume_spurious_instantiation_limits() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-recursive-format"),
    );
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (file, source, markers) in [
            (
                "cases.ts",
                SOURCE,
                &[
                    "object",
                    "nested",
                    "array",
                    "tuple",
                    "any",
                    "unknown",
                    "primitive",
                    "generic",
                    "relation",
                    "relationGeneric",
                    "contextual",
                ][..],
            ),
            (
                "variants.ts",
                VARIANTS,
                &["ObjectOnly", "NoFlatten", "Nonrecursive"][..],
            ),
        ] {
            for marker in markers {
                for expand in [false, true] {
                    let tag = format!("/*#{marker}*/");
                    let request = TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path(file),
                            position: position(source, source.find(&tag).unwrap() + tag.len()),
                        },
                        expand,
                        depth: 1,
                    };
                    let answer = project.type_of(request);
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{config}/{file}/{marker}/{expand}: {answer:?}")
                    };
                    assert_eq!(result.outcome, TypeOutcome::Found);
                    assert!(result.issues.is_empty());
                    let expected = if marker.starts_with("relation") {
                        "number"
                    } else if *marker == "contextual" || file == "variants.ts" {
                        "string"
                    } else {
                        "string[]"
                    };
                    assert_eq!(
                        result.description.summary, expected,
                        "{config}/{file}/{marker}/{expand}"
                    );
                }
            }
        }
    }
}

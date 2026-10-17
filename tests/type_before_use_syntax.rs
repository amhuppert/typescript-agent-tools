//! Compiler-derived final queries exercise the whole before-use syntax owner.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-before-use-syntax/cases.ts");
#[test]
fn native_declaration_order_and_deferred_contexts_preserve_final_access_types() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-before-use-syntax/tsconfig.json"),
        ),
        (
            "legacy.json",
            include_str!("../fixtures/types/native-before-use-syntax/legacy.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-before-use-syntax/loose.json"),
        ),
    ]);
    for config in ["tsconfig.json", "legacy.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [
            ("earlier", "number"),
            ("deferred", "number"),
            ("iife", "number"),
            ("asyncIife", "number"),
            ("generatorIife", "number"),
            ("arrow", "number"),
            ("shadow", "number"),
            ("computed", "\"a\""),
            ("self", "number"),
            ("selfArrow", "number"),
            ("parameterProperty", "number"),
            ("staticArrow", "number"),
            (
                "optional",
                if config == "loose.json" {
                    "number"
                } else {
                    "number | undefined"
                },
            ),
            ("asserted", "number"),
            ("nested", "number"),
            ("method", "() => number"),
            ("inherited", "number"),
            ("decorator", "number"),
            ("decoratorArrow", "number"),
        ] {
            let label = format!("/*#{marker}*/");
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, SOURCE.find(&label).unwrap() + label.len()),
                });
                request.expand = expand;
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}")
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{marker}");
                assert_eq!(
                    result.description.summary, summary,
                    "{config}/{marker}/{expand}"
                );
            }
        }
    }
}

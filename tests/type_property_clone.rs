//! Canonical this members require a fresh anonymous method on the union clone.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-property-clone/cases.ts");

#[test]
fn canonical_and_transient_method_clones_reach_final_access_types() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-property-clone/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-property-clone/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-property-clone/exact.json"),
        ),
    ]);
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [
            ("classMethod", "() => number"),
            ("classPlain", "() => number"),
            ("classTransient", "() => number"),
            ("interfaceMethod", "() => number"),
            ("interfacePlain", "() => number"),
            ("classField", "number"),
            ("interfaceField", "number"),
            ("classAccessor", "number"),
            ("interfaceAccessor", "number"),
            ("requiredFirst", "number"),
            (
                "optionalFirst",
                if config == "loose.json" {
                    "number"
                } else {
                    "number | undefined"
                },
            ),
            (
                "classOptional",
                if config == "loose.json" {
                    "() => number"
                } else {
                    "(() => number) | undefined"
                },
            ),
            (
                "interfaceOptional",
                if config == "loose.json" {
                    "() => number"
                } else {
                    "(() => number) | undefined"
                },
            ),
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
                    panic!("{config}/{marker}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{marker}");
                assert_eq!(result.description.summary, summary, "{config}/{marker}");
                assert!(
                    result.issues.is_empty(),
                    "{config}/{marker}: {:?}",
                    result.issues
                );
                if expand {
                    assert!(result.description.expansion.is_some(), "{config}/{marker}");
                }
            }
        }
    }
}

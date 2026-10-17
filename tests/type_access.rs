//! Access expressions consume canonical member and index questions.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-access/cases.ts");

#[test]
fn native_property_and_element_reads_preserve_keys_and_options() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-access/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-access/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-access/loose.json"),
        ),
        (
            "unchecked.json",
            include_str!("../fixtures/types/native-access/unchecked.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-access/exact.json"),
        ),
        (
            "dot-index.json",
            include_str!("../fixtures/types/native-access/dot-index.json"),
        ),
    ]);
    for config in [
        "tsconfig.json",
        "loose.json",
        "unchecked.json",
        "exact.json",
        "dot-index.json",
    ] {
        let project = fixture.load_config(config);
        let optional = if config == "loose.json" {
            ""
        } else {
            " | undefined"
        };
        let unchecked = if config == "unchecked.json" {
            " | undefined"
        } else {
            ""
        };
        let mut answers: Vec<(&str, String)> = [
            ("dot", "number"),
            ("literal", "number"),
            ("template", "number"),
            ("escaped", "number"),
            ("empty", "bigint"),
            ("decoded", "boolean"),
            ("identifier", "number"),
            ("union", "number"),
            ("fixed", "42"),
            ("chainKnown", "42"),
            ("chainAny", "any"),
            ("anyDot", "any"),
            ("anyElement", "any"),
        ]
        .into_iter()
        .map(|(name, summary)| (name, summary.into()))
        .collect();
        answers.extend([
            ("optional", format!("string{optional}")),
            ("optionalElement", format!("string{optional}")),
            ("chain", format!("number{optional}")),
            ("elementChain", format!("number{optional}")),
            ("stringIndex", format!("string | number{unchecked}")),
            ("stringElement", format!("string | number{unchecked}")),
            ("numberIndex", format!("number{unchecked}")),
            ("numericName", format!("number{unchecked}")),
            ("nonnumericName", format!("string | number{unchecked}")),
            ("specialNumericName", format!("number{unchecked}")),
        ]);
        for (name, summary) in answers {
            for expand in [false, true] {
                let offset = SOURCE.find(&format!("const {name} =")).unwrap() + 6;
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, offset),
                });
                request.expand = expand;
                request.depth = 2;
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}: actual native access required: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{name}");
                assert_eq!(result.description.summary, summary, "{config}/{name}");
                assert!(
                    result.issues.is_empty(),
                    "{config}/{name}: unused latent member"
                );
            }
        }
    }
}

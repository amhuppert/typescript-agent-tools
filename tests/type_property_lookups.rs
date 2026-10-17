//! Native index applicability is observed through the final assignment read.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-property-lookups/cases.ts");

#[test]
fn exact_numeric_names_select_number_indexes_before_string_fallback() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "literals.ts",
            include_str!("../fixtures/types/native-property-lookups/literals.ts"),
        ),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-property-lookups/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-property-lookups/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-property-lookups/loose.json"),
        ),
        (
            "unchecked.json",
            include_str!("../fixtures/types/native-property-lookups/unchecked.json"),
        ),
    ]);
    let names: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-property-lookups/names.json"
    ))
    .unwrap();
    for config in ["tsconfig.json", "loose.json", "unchecked.json"] {
        let project = fixture.load_config(config);
        for row in &names {
            let tag = format!("/*#{}*/", row["marker"].as_str().unwrap());
            let offset = SOURCE.find(&tag).unwrap() + tag.len();
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, offset),
                });
                request.expand = expand;
                request.depth = 2;
                let answer = project.type_of(request);
                if row["numeric"].as_bool().unwrap() && config != "unchecked.json" {
                    // The compiler narrows to number. T must establish that
                    // excluded flow dependency, never return the declared union.
                    assert!(
                        matches!(&answer, Err(AnalysisError::Semantic(error))
                        if error.kind == SemanticErrorKind::UnsupportedSemantic
                            && error.reason == SemanticFailureReason::FlowRequired.into()),
                        "{config}/{row:?}: expected actual number assignment flow: {answer:?}"
                    );
                } else {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!(
                            "{config}/{row:?}: string fallback preserves declared identity: {answer:?}"
                        );
                    };
                    assert_eq!(result.outcome, TypeOutcome::Found);
                    assert_eq!(result.description.summary, "string | number");
                    assert!(result.issues.is_empty());
                }
            }
        }
    }
}

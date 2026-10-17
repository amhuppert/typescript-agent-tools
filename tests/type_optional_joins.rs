//! Completed read admission permits exact intermediate facts to rejoin.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/optional-joins/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/optional-joins/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/optional-joins/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/optional-joins/loose.json"),
        ),
    ])
}

#[test]
fn optionality_and_truthiness_join_types_are_admitted_only_after_final_identity() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        let optional = if config == "loose.json" {
            ""
        } else {
            " | undefined"
        };
        for (name, base, narrowed) in [
            ("afterDot", "Box", false),
            ("afterElement", "Box", false),
            ("afterCall", "Handler", false),
            ("afterKeyEffect", "Box", false),
            ("afterReceiverEffect", "Box", false),
            ("falsyArm", "Box", false),
            ("afterBranch", "Box", false),
            ("narrowedArm", "Box", true),
            ("afterNullish", "string", false),
            ("afterTruthy", "string", false),
            ("truthyArm", "string", true),
        ] {
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(
                        SOURCE,
                        SOURCE.find(&format!("const {name} =")).unwrap() + 6,
                    ),
                });
                request.expand = expand;
                let answer = project.type_of(request);
                if narrowed && config == "tsconfig.json" {
                    assert!(
                        matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::UnsupportedSemantic
                        && error.reason == SemanticFailureReason::FlowRequired.into()),
                        "{config}/{name}: {answer:?}"
                    );
                } else {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{config}/{name}: {answer:?}");
                    };
                    assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{name}");
                    assert_eq!(
                        result.description.summary,
                        format!("{base}{optional}"),
                        "{config}/{name}"
                    );
                }
            }
        }
    }
}

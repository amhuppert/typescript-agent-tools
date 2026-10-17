//! Native before-use diagnostics remain ordinary answers; consumed synthetic
//! static-block flow must pass the same final-identity admission as other reads.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-before-use/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-before-use/tsconfig.json"),
        ),
        (
            "legacy.json",
            include_str!("../fixtures/types/native-before-use/legacy.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-before-use/loose.json"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let label = format!("/*#{marker}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&label).unwrap() + label.len()),
    });
    request.expand = expand;
    request
}

#[test]
fn native_before_use_false_diagnostics_and_identity_static_flow_keep_healthy_types() {
    let fixture = fixture();
    for config in ["tsconfig.json", "legacy.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [
            ("noBlocks", "number"),
            ("emptyBlock", "number"),
            ("anyProperty", "any"),
            ("unknownProperty", "unknown"),
            ("laterBlock", "number"),
            ("earlierDeclaration", "number"),
            ("staticMethod", "() => number"),
            ("instanceDeferred", "number"),
            ("selfReference", "number"),
        ] {
            if config == "loose.json"
                && ["emptyBlock", "anyProperty", "unknownProperty"].contains(&marker)
            {
                continue;
            }
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{marker}");
                assert_eq!(result.description.summary, summary, "{config}/{marker}");
            }
        }
    }
}

#[test]
fn before_use_static_initialization_requires_flow_when_final_type_differs_from_initial() {
    let fixture = fixture();
    for config in ["tsconfig.json", "legacy.json"] {
        let project = fixture.load_config(config);
        for marker in ["initialized", "alreadyOptional", "optionalInitialized"] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error))
                    if error.kind == SemanticErrorKind::UnsupportedSemantic
                    && error.reason == SemanticFailureReason::FlowRequired.into()),
                    "{config}/{marker}: {answer:?}"
                );
            }
        }
    }
}

#[test]
fn loose_static_block_queries_preserve_the_native_unavailable_optional_type_boundary() {
    let fixture = fixture();
    let project = fixture.load_config("loose.json");
    for marker in [
        "emptyBlock",
        "initialized",
        "alreadyOptional",
        "optionalInitialized",
        "anyProperty",
        "unknownProperty",
    ] {
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, marker, expand));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::UnsupportedSemantic
                && error.reason == SemanticFailureReason::UnsupportedSemanticForm.into()),
                "{marker}: {answer:?}"
            );
        }
    }
}

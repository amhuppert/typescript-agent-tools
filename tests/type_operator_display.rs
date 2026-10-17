//! Equality's diagnostic display consumes lazy types even when its result is Boolean.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-operator-display/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("native-operator-display/cases.ts", SOURCE),
        (
            "native-array-expressions/globals.d.ts",
            include_str!("../fixtures/types/native-array-expressions/globals.d.ts"),
        ),
        (
            "native-operator-display/tsconfig.json",
            include_str!("../fixtures/types/native-operator-display/tsconfig.json"),
        ),
        (
            "native-operator-display/loose.json",
            include_str!("../fixtures/types/native-operator-display/loose.json"),
        ),
        (
            "native-operator-display/exact.json",
            include_str!("../fixtures/types/native-operator-display/exact.json"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let marker = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("native-operator-display/cases.ts"),
            position: position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len()),
        },
        expand,
        depth: 1,
    }
}

#[test]
fn equality_reporting_retains_the_consumed_operand_annotation_failure() {
    let fixture = fixture();
    let path = fixture
        .path("native-operator-display/cases.ts")
        .canonicalize()
        .unwrap();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-operator-display/{config}.json"));
        for marker in [
            "left",
            "right",
            "notEqual",
            "looseEqual",
            "looseNotEqual",
            "initializer",
            "awaited",
            "awaitedReverse",
        ] {
            let missing = if marker.starts_with("awaited") {
                "MissingAwaited"
            } else {
                "Missing"
            };
            let expected = position(SOURCE, SOURCE.find(missing).unwrap());
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error))
                    if error.kind == SemanticErrorKind::InvalidSemantic
                    && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
                    && error.path.as_deref() == Some(path.as_path())
                    && error.position == Some(expected)),
                    "{config}/{marker}/{expand}: {answer:?}"
                );
            }
        }
    }
}

#[test]
fn compatible_or_healthy_operands_preserve_the_checked_call_result() {
    let fixture = fixture();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-operator-display/{config}.json"));
        for marker in ["same", "literals", "healthy", "optionalOnly", "quick"] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{marker}: {result:?}"
                );
                assert_eq!(result.description.summary, "\"checked\"");
                assert!(result.issues.is_empty());
            }
        }
    }
}

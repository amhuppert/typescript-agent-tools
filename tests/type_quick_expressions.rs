//! Quick expression acquisition and checked await preserve distinct demands.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-quick-expressions/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-quick-expressions/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-quick-expressions/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-quick-expressions/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-quick-expressions/exact.json"),
        ),
    ])
}
fn request(fixture: &Fixture, name: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{name}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth: 0,
    }
}
#[test]
fn quick_assertions_calls_and_awaits_skip_unrequested_dependencies() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (name, summary) in [
            ("literal", "1"),
            ("boolean", "true"),
            ("parenthesized", "1"),
            ("assertion", "number"),
            ("parenthesizedAssertion", "string"),
            ("call", "number"),
            ("awaitLiteral", "1"),
            ("awaitAssertion", "string"),
            ("awaitCall", "number"),
            ("awaitPromise", "number"),
            ("awaitScalar", "number"),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, name, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{name}: {:?}",
                    result.issues
                );
                assert_eq!(result.description.summary, summary, "{config}/{name}");
                assert!(
                    result.issues.is_empty(),
                    "{config}/{name}: {:?}",
                    result.issues
                );
            }
        }
    }
}
#[test]
fn checked_await_and_const_assertion_keep_consumed_failures() {
    let fixture = fixture();
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (name, reason, cause) in [
            ("awaitBad", SemanticFailureReason::ErrorType, "await bad"),
            (
                "constFailure",
                SemanticFailureReason::UnresolvedSymbol,
                "MissingConstOperand",
            ),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, name, expand));
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error)) if
                    error.kind == SemanticErrorKind::InvalidSemantic && error.reason == reason.into()
                    && error.path.as_deref() == Some(path.as_path())
                    && error.position == Some(position(SOURCE, SOURCE.find(cause).unwrap()))),
                    "{config}/{name}: {answer:?}"
                );
            }
        }
        let answer = project.type_of(request(&fixture, "awaitPromise", false));
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!("{config}: {answer:?}");
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(result.description.summary, "number");
        assert!(result.issues.is_empty());
    }
}

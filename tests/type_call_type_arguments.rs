//! Calls source-check explicit type arguments before overload selection.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-call-type-arguments/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-call-type-arguments/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-call-type-arguments/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-call-type-arguments/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-call-type-arguments/exact.json"),
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
fn diagnostic_only_rejections_and_quick_call_bypass_keep_native_results() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (name, summary) in [
            ("valid", "1"),
            ("rejected", "\"wrong\""),
            ("quick", "number"),
            ("untypedValid", "any"),
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
                assert!(result.issues.is_empty());
            }
        }
    }
}
#[test]
fn checked_typed_untyped_and_spread_calls_preserve_original_constraint_failure() {
    let fixture = fixture();
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (name, dependency) in [
            ("hidden", "MissingCallConstraint"),
            ("later", "MissingLaterCallConstraint"),
            ("untypedHidden", "MissingCallConstraint"),
            ("spreadHidden", "MissingCallConstraint"),
            ("firstFailure", "MissingCallConstraint"),
            ("hidden", "MissingCallConstraint"),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, name, expand));
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error)) if
                    error.kind == SemanticErrorKind::InvalidSemantic
                    && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
                    && error.path.as_deref() == Some(path.as_path())
                    && error.position == Some(position(SOURCE, SOURCE.find(dependency).unwrap()))),
                    "{config}/{name}: {answer:?}"
                );
            }
        }
        let answer = project.type_of(request(&fixture, "valid", false));
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!("{config}: {answer:?}");
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(result.description.summary, "1");
    }
}

//! Assertion/satisfies checks preserve demanded diagnostic dependencies.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-type-checks/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("native-type-checks/cases.ts", SOURCE),
        (
            "native-array-expressions/globals.d.ts",
            include_str!("../fixtures/types/native-array-expressions/globals.d.ts"),
        ),
        (
            "native-type-checks/tsconfig.json",
            include_str!("../fixtures/types/native-type-checks/tsconfig.json"),
        ),
        (
            "native-type-checks/loose.json",
            include_str!("../fixtures/types/native-type-checks/loose.json"),
        ),
        (
            "native-type-checks/exact.json",
            include_str!("../fixtures/types/native-type-checks/exact.json"),
        ),
    ])
}
fn request(fixture: &Fixture, name: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{name}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("native-type-checks/cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth: 1,
    }
}
#[test]
fn diagnostic_only_constraint_rejection_preserves_the_native_type() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(&format!("native-type-checks/{config}"));
        for (name, summary) in [
            ("valid", "1"),
            ("rejected", "string"),
            ("hidden", "number"),
            ("later", "number"),
            ("skipped", "number"),
            ("query", "number"),
            ("satisfied", "1"),
            ("named", "[value: 1]"),
            ("indexed", "1"),
            ("angle", "1"),
            ("assertionOrder", "number"),
            ("checkedRejected", "string"),
            ("checkedSkipped", "number"),
            ("checkedFunction", "(value: number) => string"),
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
fn source_check_dependencies_keep_the_original_failure_after_rejection() {
    let fixture = fixture();
    let path = fixture
        .path("native-type-checks/cases.ts")
        .canonicalize()
        .unwrap();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(&format!("native-type-checks/{config}"));
        for (name, dependency) in [
            ("checkedHidden", "MissingConstraint"),
            ("checkedLater", "MissingLaterConstraint"),
            ("argument", "MissingArgument"),
            ("satisfiesHidden", "MissingConstraint"),
            ("checkedHidden", "MissingConstraint"),
            ("checkedAssertionOrder", "MissingCheckedOperand"),
            ("satisfiesOrder", "MissingConstraint"),
            ("checkedAngle", "MissingConstraint"),
            ("checkedMissingFunction", "MissingParameter"),
            ("checkedMissingConstructor", "MissingConstructorParameter"),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, name, expand));
                assert!(
                    matches!(&answer,Err(AnalysisError::Semantic(error)) if error.kind==SemanticErrorKind::InvalidSemantic && error.reason==SemanticFailureReason::UnresolvedSymbol.into() && error.path.as_deref()==Some(path.as_path()) && error.position==Some(position(SOURCE,SOURCE.find(dependency).unwrap()))),
                    "{config}/{name}: {answer:?}"
                );
            }
        }
    }
}

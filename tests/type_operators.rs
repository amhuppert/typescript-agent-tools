//! Final operator answers and consumed failures against the pinned native matrix.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-operators/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-operators/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-operators/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-operators/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-operators/exact.json"),
        ),
    ])
}

fn check_family(include: impl Fn(&str) -> bool) {
    let fixture = fixture();
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (config, expected) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-operators/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-operators/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-operators/expected-exact.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("{config}.json"));
        let rows: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        let mut checked = 0;
        for row in rows {
            let name = row["name"].as_str().unwrap();
            if !include(name) {
                continue;
            }
            checked += 1;
            let marker = format!("/*#{name}*/");
            let selected = SOURCE.find(&marker).unwrap() + marker.len();
            let initializer = selected + SOURCE[selected..].find(" = ").unwrap() + 3;
            let missing = !row["symbolError"].is_null();
            let unknown = config != "loose"
                && matches!(name, "unknownSubtract" | "unknownAdd" | "minusUnknown");
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, selected),
                });
                request.expand = expand;
                request.depth = 1;
                let answer = project.type_of(request);
                if missing || unknown || row["errorType"] == true {
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{config}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{config}/{name}: {error:?}"
                    );
                    let reason = if missing {
                        SemanticFailureReason::UnresolvedSymbol
                    } else {
                        SemanticFailureReason::ErrorType
                    };
                    assert_eq!(error.reason, reason.into(), "{config}/{name}: {error:?}");
                    assert_eq!(
                        error.path.as_deref(),
                        Some(path.as_path()),
                        "{config}/{name}"
                    );
                    let failure = if missing {
                        SOURCE.find("Missing").unwrap()
                    } else {
                        initializer + usize::from(name == "minusUnknown")
                    };
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, failure)),
                        "{config}/{name}: {error:?}"
                    );
                } else {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{config}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        result.outcome,
                        TypeOutcome::Found,
                        "{config}/{name}: {result:?}"
                    );
                    assert_eq!(
                        result.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{config}/{name}/{expand}"
                    );
                    assert!(result.issues.is_empty(), "{config}/{name}: {result:?}");
                }
            }
        }
        assert!(checked > 0);
    }
}

fn assignment(name: &str) -> bool {
    name.starts_with("compound") || name.starts_with("assign")
}
fn unary(name: &str) -> bool {
    name.starts_with("prefix")
        || name.starts_with("postfix")
        || name.starts_with("not")
        || name.starts_with("minus")
        || name.starts_with("plus")
        || name.starts_with("tilde")
        || name == "voidMissing"
}

#[test]
fn arithmetic_comparison_and_constant_evaluation_keep_native_results_and_failures() {
    check_family(|name| !assignment(name) && !unary(name));
}

#[test]
fn unary_operators_check_constraints_and_preserve_literal_and_numeric_domains() {
    check_family(unary);
}

#[test]
fn assignment_operators_preserve_native_result_types() {
    check_family(assignment);
}

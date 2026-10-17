//! Native diagnostic truncation affects which lazy semantic failures are read.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-diagnostic-display/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("native-diagnostic-display/cases.ts", SOURCE),
        (
            "native-array-expressions/globals.d.ts",
            include_str!("../fixtures/types/native-array-expressions/globals.d.ts"),
        ),
        (
            "native-diagnostic-display/tsconfig.json",
            include_str!("../fixtures/types/native-diagnostic-display/tsconfig.json"),
        ),
        (
            "native-diagnostic-display/true.json",
            include_str!("../fixtures/types/native-diagnostic-display/true.json"),
        ),
        (
            "native-diagnostic-display/false.json",
            include_str!("../fixtures/types/native-diagnostic-display/false.json"),
        ),
        (
            "native-diagnostic-display/inherited.json",
            include_str!("../fixtures/types/native-diagnostic-display/inherited.json"),
        ),
        (
            "native-diagnostic-display/override.json",
            include_str!("../fixtures/types/native-diagnostic-display/override.json"),
        ),
        (
            "native-diagnostic-display/reset.json",
            include_str!("../fixtures/types/native-diagnostic-display/reset.json"),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("native-diagnostic-display/cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth: 1,
    }
}
#[test]
fn native_utf8_length_boundary_and_option_inheritance_control_diagnostic_dependencies() {
    let fixture = fixture();
    let path = fixture
        .path("native-diagnostic-display/cases.ts")
        .canonicalize()
        .unwrap();
    for config in [
        "tsconfig",
        "true",
        "false",
        "inherited",
        "override",
        "reset",
    ] {
        let project = fixture.load_config(&format!("native-diagnostic-display/{config}.json"));
        for marker in [
            "under",
            "at",
            "over",
            "utf8At",
            "utf8Over",
            "first",
            "last",
            "alias",
            "healthyAfter",
            "under",
            "healthyAfter",
        ] {
            let invalid = !matches!(marker, "alias" | "healthyAfter")
                && (!matches!(marker, "over" | "utf8Over")
                    || matches!(config, "true" | "inherited"));
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                if invalid {
                    let dependency = format!("Missing_{marker}");
                    assert!(
                        matches!(&answer,Err(AnalysisError::Semantic(error)) if error.kind==SemanticErrorKind::InvalidSemantic && error.reason==SemanticFailureReason::UnresolvedSymbol.into() && error.path.as_deref()==Some(path.as_path()) && error.position==Some(position(SOURCE,SOURCE.find(&dependency).unwrap()))),
                        "{config}/{marker}/{expand}: {answer:?}"
                    );
                } else {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{config}/{marker}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        result.outcome,
                        TypeOutcome::Found,
                        "{config}/{marker}: {:?}",
                        result.issues
                    );
                    assert_eq!(result.description.summary, "number");
                    assert!(result.issues.is_empty());
                }
            }
        }
    }
}

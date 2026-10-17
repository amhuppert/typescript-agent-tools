//! Native truthiness checking preserves symbol-use and diagnostic-read boundaries.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-truthy-operators/cases.ts");

#[test]
fn truthy_checks_preserve_native_symbol_uses_and_lazy_display_dependencies() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-truthy-operators/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-truthy-operators/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-truthy-operators/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-truthy-operators/exact.json"),
        ),
    ]);
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for (profile, expected) in [
        (
            "tsconfig",
            include_str!("../fixtures/types/native-truthy-operators/expected-tsconfig.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-truthy-operators/expected-loose.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-truthy-operators/expected-exact.json"),
        ),
    ] {
        let project = fixture.load_config(&format!("{profile}.json"));
        let expected: Vec<serde_json::Value> = serde_json::from_str(expected).unwrap();
        assert_eq!(expected.len(), 25);
        for row in expected {
            let name = row["name"].as_str().unwrap();
            let marker = format!("/*#{name}*/");
            let selected = SOURCE.find(&marker).unwrap() + marker.len();
            for expand in [false, true] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(SOURCE, selected),
                });
                request.expand = expand;
                request.depth = 2;
                let answer = project.type_of(request);
                // The loose bare branch still displays the returned object,
                // even though its truthiness worker performs no diagnostic read.
                let missing = !row["symbolError"].is_null() || name == "conditionalBareUse";
                if missing {
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::InvalidSemantic,
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(error.path.as_deref(), Some(path.as_path()));
                    let offset = if name.starts_with("property") {
                        SOURCE.rfind("MissingDisplay").unwrap()
                    } else {
                        SOURCE.find("MissingDisplay").unwrap()
                    };
                    assert_eq!(
                        error.reason,
                        SemanticFailureReason::UnresolvedSymbol.into(),
                        "{profile}/{name}: {error:?}"
                    );
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, offset)),
                        "{profile}/{name}: {error:?}"
                    );
                } else {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{profile}/{name}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        result.outcome,
                        TypeOutcome::Found,
                        "{profile}/{name}: {result:?}"
                    );
                    assert_eq!(
                        result.description.summary,
                        row["summary"].as_str().unwrap(),
                        "{profile}/{name}/{expand}"
                    );
                    assert!(result.issues.is_empty(), "{profile}/{name}: {result:?}");
                }
            }
        }
    }
}

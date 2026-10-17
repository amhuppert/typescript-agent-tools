//! Expression operands inside types retain their actual value-reference context.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-reference-contexts/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("native-reference-contexts/cases.ts", SOURCE),
        (
            "native-reference-contexts/names.ts",
            include_str!("../fixtures/types/native-reference-contexts/names.ts"),
        ),
        (
            "native-reference-contexts/tsconfig.json",
            include_str!("../fixtures/types/native-reference-contexts/tsconfig.json"),
        ),
        (
            "native-array-expressions/globals.d.ts",
            include_str!("../fixtures/types/native-array-expressions/globals.d.ts"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("native-reference-contexts/cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth: 1,
    }
}

#[test]
fn value_operands_inside_types_preserve_expression_and_query_roles() {
    let fixture = fixture();
    let project = fixture.load_config("native-reference-contexts/tsconfig.json");
    for marker in [
        "property",
        "method",
        "interface",
        "class",
        "object",
        "binding",
        "initializer",
        "parameter",
        "query",
        "nestedExpression",
        "nestedQuery",
        "importedExpression",
        "importedQuery",
    ] {
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, marker, expand));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{marker}/{expand}: {answer:?}");
            };
            assert_eq!(
                result.outcome,
                TypeOutcome::Found,
                "{marker}: {:?}",
                result.issues
            );
            assert_eq!(
                result.description.summary,
                if marker.starts_with("imported") {
                    "\"imported\""
                } else {
                    "\"stable\""
                }
            );
            assert!(result.issues.is_empty());
        }
    }
}

#[test]
fn failed_value_operands_inside_types_keep_original_missing_name() {
    let fixture = fixture();
    let project = fixture.load_config("native-reference-contexts/tsconfig.json");
    let path = fixture
        .path("native-reference-contexts/cases.ts")
        .canonicalize()
        .unwrap();
    for marker in ["missingComputed", "missingInitializer"] {
        let tag = format!("/*#{marker}*/");
        let at = position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len());
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, marker, expand));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::InvalidSemantic
                && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
                && error.path.as_deref() == Some(path.as_path())
                && error.position == Some(at)),
                "{marker}/{expand}: {answer:?}"
            );
        }
    }
}

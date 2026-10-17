//! Native malformed-tag and live parser-context controls. JSDoc diagnostics
//! may recover; consumed type failures retain their actual annotation range.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const RECOVERY: &str = include_str!("../fixtures/types/native-deprecated-recovery/cases.ts");
const CONTEXT: &str = include_str!("../fixtures/types/native-deprecated-context/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("native-deprecated-recovery/cases.ts", RECOVERY),
        (
            "native-deprecated-recovery/tsconfig.json",
            include_str!("../fixtures/types/native-deprecated-recovery/tsconfig.json"),
        ),
        (
            "native-deprecated-recovery/loose.json",
            include_str!("../fixtures/types/native-deprecated-recovery/loose.json"),
        ),
        (
            "native-deprecated-recovery/exact.json",
            include_str!("../fixtures/types/native-deprecated-recovery/exact.json"),
        ),
        ("native-deprecated-context/cases.ts", CONTEXT),
        (
            "native-deprecated-context/tsconfig.json",
            include_str!("../fixtures/types/native-deprecated-context/tsconfig.json"),
        ),
        (
            "native-deprecated-context/loose.json",
            include_str!("../fixtures/types/native-deprecated-context/loose.json"),
        ),
        (
            "native-deprecated-context/exact.json",
            include_str!("../fixtures/types/native-deprecated-context/exact.json"),
        ),
        (
            "native-deprecated-signatures/globals.d.ts",
            include_str!("../fixtures/types/native-deprecated-signatures/globals.d.ts"),
        ),
    ])
}

fn request(
    fixture: &Fixture,
    directory: &str,
    source: &str,
    marker: &str,
    expand: bool,
) -> TypeRequest {
    let marker = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path(&format!("{directory}/cases.ts")),
            position: position(source, source.find(&marker).unwrap() + marker.len()),
        },
        expand,
        depth: 1,
    }
}

#[test]
fn recovered_jsdoc_grammar_still_consumes_the_following_deprecated_annotation() {
    let fixture = fixture();
    let directory = "native-deprecated-recovery";
    let path = fixture
        .path(&format!("{directory}/cases.ts"))
        .canonicalize()
        .unwrap();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("{directory}/{config}.json"));
        for marker in [
            "importString",
            "importKeyword",
            "attributeValue",
            "attributeOpen",
            "extendsUnclosed",
            "keywordChild",
            "identifierChild",
            "escapedKeywordChild",
            "cr",
            "lf",
            "crlf",
            "eagerInvalidImport",
            "eagerAttribute",
        ] {
            let expected = position(
                RECOVERY,
                RECOVERY.find(&format!("Missing_{marker}}}")).unwrap(),
            );
            for expand in [false, true] {
                let answer =
                    project.type_of(request(&fixture, directory, RECOVERY, marker, expand));
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
fn eager_jsdoc_parsing_preserves_the_enclosing_await_and_yield_context() {
    let fixture = fixture();
    let directory = "native-deprecated-context";
    let path = fixture
        .path(&format!("{directory}/cases.ts"))
        .canonicalize()
        .unwrap();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("{directory}/{config}.json"));
        for marker in [
            "ordinaryAwait",
            "asyncAwait",
            "ordinaryYield",
            "generatorYield",
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, directory, CONTEXT, marker, expand));
                if marker.starts_with("ordinary") {
                    let expected = position(
                        CONTEXT,
                        CONTEXT.find(&format!("Missing_{marker}}}")).unwrap(),
                    );
                    assert!(
                        matches!(&answer, Err(AnalysisError::Semantic(error))
                        if error.kind == SemanticErrorKind::InvalidSemantic
                        && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
                        && error.path.as_deref() == Some(path.as_path())
                        && error.position == Some(expected)),
                        "{config}/{marker}/{expand}: {answer:?}"
                    );
                } else {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{config}/{marker}/{expand}: {answer:?}");
                    };
                    assert_eq!(
                        result.outcome,
                        TypeOutcome::Found,
                        "{config}/{marker}: {result:?}"
                    );
                    assert_eq!(
                        result.description.summary,
                        format!("\"{marker}\""),
                        "{config}/{marker}"
                    );
                    assert!(result.issues.is_empty(), "{config}/{marker}: {result:?}");
                }
            }
        }
    }
}

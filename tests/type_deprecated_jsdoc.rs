//! Expected tag eligibility and hidden dependency acquisition come from the
//! pinned native JSDoc harness, including eager see/link parsing and recovery.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-deprecated-jsdoc/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("native-deprecated-jsdoc/cases.ts", SOURCE),
        (
            "native-deprecated-jsdoc/tsconfig.json",
            include_str!("../fixtures/types/native-deprecated-jsdoc/tsconfig.json"),
        ),
        (
            "native-deprecated-jsdoc/loose.json",
            include_str!("../fixtures/types/native-deprecated-jsdoc/loose.json"),
        ),
        (
            "native-deprecated-jsdoc/exact.json",
            include_str!("../fixtures/types/native-deprecated-jsdoc/exact.json"),
        ),
        (
            "native-deprecated-signatures/globals.d.ts",
            include_str!("../fixtures/types/native-deprecated-signatures/globals.d.ts"),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let marker = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("native-deprecated-jsdoc/cases.ts"),
            position: position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len()),
        },
        expand,
        depth: 1,
    }
}

#[test]
fn parsed_jsdoc_tags_consume_the_original_annotation_in_all_profiles() {
    let fixture = fixture();
    let path = fixture
        .path("native-deprecated-jsdoc/cases.ts")
        .canonicalize()
        .unwrap();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-deprecated-jsdoc/{config}.json"));
        for marker in [
            "space",
            "tab",
            "cr",
            "lf",
            "crlf",
            "brace",
            "asterisk",
            "bare",
            "eagerNbsp",
            "eagerEscaped",
            "param",
            "paramDefault",
            "paramQuoted",
            "paramNested",
            "template",
            "typedef",
            "typedefType",
            "callback",
            "overload",
            "implements",
            "augments",
            "import",
            "importDefault",
            "importNamespace",
            "satisfies",
            "throws",
            "this",
            "type",
            "unclosedType",
            "arrow",
            "fn",
            "method",
        ] {
            let missing = format!("Missing_{marker}");
            // Match the complete fixture token: Missing_type is also the
            // prefix of the earlier Missing_typedef declaration.
            let offset = SOURCE
                .match_indices(&missing)
                .find_map(|(offset, _)| {
                    matches!(
                        SOURCE.as_bytes().get(offset + missing.len()),
                        Some(b' ' | b'}')
                    )
                    .then_some(offset)
                })
                .unwrap();
            let expected = position(SOURCE, offset);
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into() && error.path.as_deref() == Some(path.as_path()) && error.position == Some(expected)),
                    "{config}/{marker}/{expand}: {answer:?}"
                );
            }
        }
    }
}

#[test]
fn scanner_hints_and_nested_comment_text_do_not_replace_tag_confirmation() {
    let fixture = fixture();
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-deprecated-jsdoc/{config}.json"));
        for marker in [
            "nbsp",
            "vt",
            "ff",
            "ls",
            "ps",
            "escaped",
            "unclosedDefault",
            "email",
            "inline",
            "fence",
            "link",
            "long",
            "hyphen",
            "typed",
            "constructed",
            "callable",
            "qualifiedNormal",
        ] {
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

#[test]
fn signature_qualified_names_consume_actual_class_type_parameter_defaults() {
    let fixture = fixture();
    let path = fixture
        .path("native-deprecated-jsdoc/cases.ts")
        .canonicalize()
        .unwrap();
    let expected = position(SOURCE, SOURCE.find("MissingQualifiedDefault").unwrap());
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("native-deprecated-jsdoc/{config}.json"));
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, "qualified", expand));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into() && error.path.as_deref() == Some(path.as_path()) && error.position == Some(expected)),
                "{config}/{expand}: {answer:?}"
            );
        }
    }
}

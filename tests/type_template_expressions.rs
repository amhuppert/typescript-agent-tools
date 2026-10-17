//! Final type requests preserve the native difference between checked literal
//! types, source constants, and contextual template construction.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn template_expressions_use_source_constants_and_native_context_in_order() {
    let source = include_str!("../fixtures/types/native-template-expressions/cases.ts");
    for (profile, config) in [
        (
            "strict",
            include_str!("../fixtures/types/native-template-expressions/tsconfig.json"),
        ),
        (
            "exact",
            include_str!("../fixtures/types/native-template-expressions/exact.json"),
        ),
        (
            "loose",
            include_str!("../fixtures/types/native-template-expressions/loose.json"),
        ),
    ] {
        let fixture = Fixture::new(&[
            ("tsconfig.json", config),
            ("cases.ts", source),
            (
                "globals.d.ts",
                include_str!("../fixtures/types/native-template-expressions/globals.d.ts"),
            ),
        ]);
        let project = fixture.load();
        for (marker, summary) in [
            ("directString", "\"abc\""),
            ("directNumber", "\"n12z\""),
            ("directBoolean", "string"),
            ("directBigint", "string"),
            ("nullValue", "string"),
            ("undefinedValue", "string"),
            ("constant", "\"known\""),
            ("constantRead", "\"aknownz\""),
            ("annotatedRead", "string"),
            ("mutableRead", "string"),
            ("parentheses", "\"abz\""),
            ("assertion", "string"),
            ("constAssertion", "\"abz\""),
            ("pattern", "`a${string}z`"),
            ("numericPattern", "`a${number}z`"),
            ("unionPattern", "\"aaz\" | \"abz\""),
            ("anyPattern", "`a${any}z`"),
            ("unknownPattern", "`a${string}z`"),
            ("objectPattern", "`a${string}z`"),
            ("symbolPattern", "`a${string}z`"),
            ("nested", "\"ab1cz\""),
            ("freshMutable", "\"a1z\""),
            ("widenedMutable", "string"),
            ("cook", "\"ab\\n\""),
            ("neverPattern", "never"),
            ("contextual", "void"),
            ("element", "number"),
            ("negativeNumber", "\"a-1z\""),
            ("constBoolean", "\"btruez\""),
            ("constBigint", "\"b12z\""),
        ] {
            let tag = format!("/*#{marker}*/");
            let answer = project.type_of(TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(source, source.find(&tag).unwrap() + tag.len()),
            }));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(result)) if
                result.outcome == TypeOutcome::Found && result.description.summary == summary),
                "{profile} {marker}: {answer:?}"
            );
        }
    }
}

#[test]
fn a_failed_substitution_is_consumed_before_template_widens_to_string() {
    let source = include_str!("../fixtures/types/native-template-expressions/cases.ts");
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-template-expressions/tsconfig.json"),
        ),
        ("cases.ts", source),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-template-expressions/globals.d.ts"),
        ),
    ]);
    let project = fixture.load();
    let tag = "/*#missing*/";
    let answer = project.type_of(TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(source, source.find(tag).unwrap() + tag.len()),
    }));
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error)) if
        error.kind == SemanticErrorKind::InvalidSemantic &&
        error.reason == SemanticFailureReason::UnresolvedSymbol.into() &&
        error.path.as_deref() == Some(path.as_path()) &&
        error.position == Some(position(source, source.find("MissingPlaceholder").unwrap()))),
        "{answer:?}"
    );
}

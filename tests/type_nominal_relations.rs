//! Final overload consumers for native nominal property comparison.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const DEFINITIONS: &str = include_str!("../fixtures/types/native-nominal-relations/definitions.ts");
const SITES: &str = include_str!("../fixtures/types/native-nominal-relations/sites.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-nominal-relations/tsconfig.json"),
        ),
        ("definitions.ts", DEFINITIONS),
        ("sites.ts", SITES),
    ])
}
fn selected(name: &str, expected: &str) {
    let fixture = fixture();
    let answer = fixture.load().type_of(TypeRequest::new(TypeQuery::At {
        file: fixture.path("sites.ts"),
        position: position(SITES, SITES.find(&format!("{name}Result =")).unwrap()),
    }));
    let Ok(TypeAnswer::Selected(result)) = &answer else {
        panic!("{name}: {answer:?}");
    };
    assert_eq!(result.outcome, TypeOutcome::Found, "{name}: {result:?}");
    assert!(result.issues.is_empty(), "{name}: {result:?}");
    assert_eq!(
        result.description.summary,
        format!("\"{expected}\""),
        "{name}"
    );
}
#[test]
fn protected_inherited() {
    selected("protectedInherited", "chosen");
}

#[test]
fn protected_override() {
    selected("protectedOverride", "chosen");
}

#[test]
fn protected_public_override() {
    selected("protectedPublicOverride", "chosen");
}

#[test]
fn protected_transitive() {
    selected("protectedTransitive", "chosen");
}

#[test]
fn protected_unrelated() {
    selected("protectedUnrelated", "same");
}

#[test]
fn protected_structural() {
    selected("protectedStructural", "same");
}

#[test]
fn protected_to_public() {
    selected("protectedToPublic", "same");
}

#[test]
fn private_inherited() {
    selected("privateInherited", "chosen");
}

#[test]
fn private_separate() {
    selected("privateSeparate", "same");
}

#[test]
fn private_to_public() {
    selected("privateToPublic", "same");
}

#[test]
fn public_to_private() {
    selected("publicToPrivate", "same");
}

#[test]
fn private_generic_same() {
    selected("privateGenericSame", "chosen");
}

#[test]
fn private_generic_different() {
    selected("privateGenericDifferent", "same");
}

#[test]
fn protected_generic() {
    selected("protectedGeneric", "chosen");
}

#[test]
fn nominal_skipped_source() {
    selected("nominalSkippedSource", "same");
}

#[test]
fn nominal_skipped_target() {
    selected("nominalSkippedTarget", "same");
}

#[test]
fn nominal_comparison_preserves_a_consumed_value_failure() {
    let fixture = fixture();
    let path = fixture.path("definitions.ts").canonicalize().unwrap();
    let answer = fixture.load().type_of(TypeRequest::new(TypeQuery::At {
        file: fixture.path("sites.ts"),
        position: position(SITES, SITES.find("nominalConsumedSourceResult =").unwrap()),
    }));
    assert!(
        matches!(&answer,Err(AnalysisError::Semantic(error))
        if error.kind == SemanticErrorKind::InvalidSemantic
            && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
            && error.path.as_deref() == Some(path.as_path())
            && error.position == Some(position(DEFINITIONS, DEFINITIONS.find("MissingNominalConsumed").unwrap()))),
        "{answer:?}"
    );
}

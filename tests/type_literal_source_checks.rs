//! Native TypeLiteral checking includes consumed diagnostic dependencies.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-type-literal-checks/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("native-type-literal-checks/cases.ts", SOURCE),
        (
            "native-array-expressions/globals.d.ts",
            include_str!("../fixtures/types/native-array-expressions/globals.d.ts"),
        ),
        (
            "native-type-literal-checks/tsconfig.json",
            include_str!("../fixtures/types/native-type-literal-checks/tsconfig.json"),
        ),
        (
            "native-type-literal-checks/loose.json",
            include_str!("../fixtures/types/native-type-literal-checks/loose.json"),
        ),
        (
            "native-type-literal-checks/exact.json",
            include_str!("../fixtures/types/native-type-literal-checks/exact.json"),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("native-type-literal-checks/cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth: 1,
    }
}
#[test]
fn diagnostic_only_type_literal_rejections_keep_native_result() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(&format!("native-type-literal-checks/{config}"));
        for marker in [
            "plain",
            "optional",
            "explicitUndefined",
            "patternMatch",
            "patternMiss",
            "patternRejected",
            "stringNumber",
            "symbol",
            "duplicateIndex",
            "unionIndex",
            "literalKey",
            "invalidKey",
            "intersectionKey",
            "duplicateProperty",
            "duplicateAccessor",
            "propertyAccessor",
            "overloads",
            "computedLiteral",
            "computedStable",
            "nested",
            "quick",
            "displayColdNoIndex",
            "displayNonmatchingIndex",
            "bodylessInitializer",
            "healthyAfter",
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{marker}: {:?}",
                    result.issues
                );
                assert_eq!(
                    result.description.summary,
                    if marker == "quick" {
                        "{ value: number; }"
                    } else {
                        "number"
                    }
                );
                assert!(result.issues.is_empty());
            }
        }
    }
}
#[test]
fn type_literal_source_and_diagnostic_reads_preserve_original_failure_order() {
    let fixture = fixture();
    let path = fixture
        .path("native-type-literal-checks/cases.ts")
        .canonicalize()
        .unwrap();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(&format!("native-type-literal-checks/{config}"));
        for (marker, dependency) in [
            ("propertyConstraint", "MissingPropertyConstraint"),
            ("propertyFirst", "MissingPropertyConstraint"),
            ("duplicateThenMissing", "MissingLaterPropertyConstraint"),
            ("propertyRaw", "MissingPropertyRaw"),
            ("indexKeyFirst", "MissingIndexConstraint"),
            ("indexValue", "MissingReturnConstraint"),
            ("invalidKeyValue", "MissingReturnConstraint"),
            ("duplicateIndexLater", "MissingReturnConstraint"),
            ("methodParameter", "MissingMethodConstraint"),
            ("methodReturn", "MissingMethodConstraint"),
            ("getterReturn", "MissingMethodConstraint"),
            ("setterParameter", "MissingMethodConstraint"),
            ("computedBeforeAnnotation", "MissingComputedName"),
            ("computedBinaryAfterAnnotation", "MissingPropertyConstraint"),
            ("displayPropertyMismatch", "MissingDiagnosticDisplay"),
            ("displayIndexMismatch", "MissingDiagnosticDisplay"),
            ("displaySecondaryMismatch", "MissingDiagnosticDisplay"),
            ("initializedProperty", "MissingPropertyInitializer"),
            ("indexInitializer", "MissingIndexConstraint"),
            ("indexRest", "MissingReturnConstraint"),
            ("displayPropertyMismatch", "MissingDiagnosticDisplay"),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind==SemanticErrorKind::InvalidSemantic && error.reason==SemanticFailureReason::UnresolvedSymbol.into() && error.path.as_deref()==Some(path.as_path()) && error.position==Some(position(SOURCE,SOURCE.find(dependency).unwrap()))),
                    "{config}/{marker}/{expand}: {answer:?}"
                );
            }
        }
        let answer = project.type_of(request(&fixture, "healthyAfter", false));
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome==TypeOutcome::Found && result.description.summary=="number"),
            "{config}: {answer:?}"
        );
    }
}

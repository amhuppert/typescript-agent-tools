//! Native index-key admission is observed through actual lookup and keyof queries.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-template-indexes/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-template-indexes/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-template-indexes/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-template-indexes/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-template-indexes/exact.json"),
        ),
        (
            "unchecked.json",
            include_str!("../fixtures/types/native-template-indexes/unchecked.json"),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth: 0,
    }
}
#[test]
fn index_key_families_keep_native_lookup_and_rejection_results() {
    let fixture = fixture();
    let expected = [
        ("primitiveString", "\"string\""),
        ("primitiveNumber", "\"number\""),
        ("primitiveSymbol", "\"symbol\""),
        ("patternNumber", "\"pattern\""),
        ("patternBigInt", "\"big\""),
        ("patternString", "\"str\""),
        ("mappingUpper", "\"upper\""),
        ("mappingNested", "\"nested\""),
        ("mappingTemplate", "\"mapped-template\""),
        ("brandString", "\"brand-string\""),
        ("brandNumber", "\"brand-number\""),
        ("brandPattern", "\"brand-pattern\""),
        ("intersectionRejected", "never"),
        ("literalRejected", "never"),
        ("anyRejected", "never"),
        ("booleanPatternRejected", "never"),
        ("unionPrimitive", "\"all\""),
        ("unionPatterns", "\"union\""),
        ("mixedUnionKey", "\"valid-part\""),
        ("overlapFallback", "\"pattern\""),
        ("overlapPatterns", "never"),
        ("duplicate", "\"first\""),
        ("duplicateUnion", "\"first\""),
        ("readonlyPattern", "\"readonly\""),
        ("genericRejected", "never"),
        ("genericPatternRejected", "never"),
        ("genericMappingRejected", "never"),
        ("patternExpression", "\"pattern\""),
    ];
    for config in [
        "tsconfig.json",
        "loose.json",
        "exact.json",
        "unchecked.json",
    ] {
        let project = fixture.load_config(config);
        for (marker, summary) in expected {
            let summary = if marker == "patternExpression" && config == "unchecked.json" {
                "\"pattern\" | undefined"
            } else {
                summary
            };
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
                assert_eq!(result.description.summary, summary, "{config}/{marker}");
                assert!(result.issues.is_empty());
            }
        }
    }
}
#[test]
fn nonmatching_pattern_and_mapping_keys_keep_index_failure() {
    let fixture = fixture();
    for config in [
        "tsconfig.json",
        "loose.json",
        "exact.json",
        "unchecked.json",
    ] {
        let project = fixture.load_config(config);
        for marker in ["patternBad", "mappingBad"] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::ErrorType.into()),
                    "{config}/{marker}: {answer:?}"
                );
            }
        }
    }
}
#[test]
fn index_values_are_consumed_before_invalid_or_duplicate_key_filtering() {
    let fixture = fixture();
    let path = fixture.path("cases.ts").canonicalize().unwrap();
    for config in [
        "tsconfig.json",
        "loose.json",
        "exact.json",
        "unchecked.json",
    ] {
        let project = fixture.load_config(config);
        for (marker, dependency) in [
            ("duplicateFailure", "MissingDuplicateValue"),
            ("invalidKeyFailure", "MissingInvalidKeyValue"),
            ("valueBeforeKey", "MissingIndexValue"),
            ("missingKey", "MissingKeyOnly"),
            ("duplicateFailure", "MissingDuplicateValue"),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into() && error.path.as_deref() == Some(path.as_path()) && error.position == Some(position(SOURCE,SOURCE.find(dependency).unwrap()))),
                    "{config}/{marker}: {answer:?}"
                );
            }
        }
        let answer = project.type_of(request(&fixture, "patternNumber", false));
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!("{config}: {answer:?}");
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(result.description.summary, "\"pattern\"");
    }
}

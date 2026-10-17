//! Reporting relations retain semantic reads even when native diagnostics
//! are later discarded or a rejected instantiation keeps its signature.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-reporting-relations/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-reporting-relations/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-reporting-relations/tsconfig.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-reporting-relations/exact.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-reporting-relations/loose.json"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
    });
    request.expand = expand;
    request
}

fn healthy(cases: &[(&str, &str)]) {
    let fixture = fixture();
    for config in ["tsconfig.json", "exact.json", "loose.json"] {
        let project = fixture.load_config(config);
        for &(marker, expected) in cases {
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
                    result.description.summary, expected,
                    "{config}/{marker}/{expand}"
                );
                assert!(
                    result.issues.is_empty(),
                    "{config}/{marker}: {:?}",
                    result.issues
                );
            }
        }
    }
}

#[test]
fn silent_constraints_skip_reporting_only_dependencies() {
    healthy(&[
        ("weakCallSilent", "\"fallback\""),
        ("weakConstructSilent", "\"fallback\""),
        ("weakBothSilent", "\"fallback\""),
        ("weakSecondSilent", "\"fallback\""),
        ("overloadSilent", "\"matched\""),
        ("cacheSilent", "\"fallback\""),
    ]);
}

#[test]
fn compatible_call_return_skips_the_construct_return_during_reporting() {
    healthy(&[(
        "weakBothReport",
        "{ <T extends Weak>(): \"matched\"; (): \"fallback\"; }",
    )]);
}

#[test]
fn ordinary_reporting_rejections_preserve_the_native_generic_signature() {
    healthy(&[
        ("rejected", "<T extends string>() => \"matched\""),
        ("accepted", "() => \"matched\""),
        (
            "tupleReadonly",
            "<T extends [number, string]>() => \"matched\"",
        ),
        (
            "tupleShort",
            "<T extends [number, string]>() => \"matched\"",
        ),
        ("tupleLong", "<T extends [number, string]>() => \"matched\""),
        (
            "indexProperty",
            "<T extends { [key: string]: number; }>() => \"matched\"",
        ),
        (
            "indexKey",
            "<T extends { [key: string]: number; }>() => \"matched\"",
        ),
        (
            "optional",
            "<T extends { value: string; }>() => \"matched\"",
        ),
        ("missingOne", "<T extends { a: number; }>() => \"matched\""),
        (
            "missingFive",
            "<T extends { a: number; b: number; c: number; d: number; e: number; }>() => \"matched\"",
        ),
        (
            "missingSix",
            "<T extends { a: number; b: number; c: number; d: number; e: number; f: number; }>() => \"matched\"",
        ),
        ("private", "<T extends Base>() => \"matched\""),
    ]);
}

#[test]
fn reporting_dependencies_survive_return_shortcuts_diagnostic_rollback_and_cache_replay() {
    let fixture = fixture();
    for config in ["tsconfig.json", "exact.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, missing) in [
            ("weakCallReport", "MissingCallReturn"),
            ("weakConstructReport", "MissingConstructReturn"),
            ("weakSecondReport", "MissingSecondConstruct"),
            ("overloadReport", "MissingFirstParameter"),
            ("cacheReport", "MissingCachedProperty"),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(
                    error.kind,
                    SemanticErrorKind::InvalidSemantic,
                    "{config}/{marker}"
                );
                assert_eq!(
                    error.reason,
                    SemanticFailureReason::UnresolvedSymbol.into(),
                    "{config}/{marker}"
                );
                assert_eq!(
                    error.path.as_deref(),
                    Some(fixture.path("cases.ts").as_path()),
                    "{config}/{marker}"
                );
                assert_eq!(
                    error.position,
                    Some(position(SOURCE, SOURCE.find(missing).unwrap())),
                    "{config}/{marker}"
                );
            }
        }
    }
}

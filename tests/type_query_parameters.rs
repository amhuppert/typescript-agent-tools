//! Native type-query ancestry and lazy raw-symbol questions through final reads.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-type-query-parameters/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "values.ts",
            include_str!("../fixtures/types/native-type-query-parameters/values.ts"),
        ),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-type-query-parameters/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-type-query-parameters/tsconfig.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-type-query-parameters/exact.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-type-query-parameters/loose.json"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
    })
}

fn check(cases: &[(&str, &str)]) {
    let fixture = fixture();
    for config in ["tsconfig.json", "exact.json", "loose.json"] {
        let project = fixture.load_config(config);
        for &(marker, expected) in cases {
            for expand in [false, true] {
                let mut query = request(&fixture, marker);
                query.expand = expand;
                let answer = project.type_of(query);
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
fn local_and_external_query_declarations_keep_native_parameter_mapping() {
    check(&[
        ("local", "string"),
        ("external", "\"external\""),
        ("qualifiedLocal", "string"),
        ("qualifiedExternal", "\"namespace\""),
        ("imported", "\"imported\""),
    ]);
}

#[test]
fn query_type_arguments_are_references_even_when_the_value_is_external() {
    check(&[("arguments", "(value: number) => number")]);
}

#[test]
fn raw_alias_unknown_and_this_reference_questions_do_not_consume_value_types() {
    check(&[("this", "\"this-healthy\"")]);
}

#[test]
fn consumed_query_failures_remain_invalid_after_raw_reference_analysis() {
    let fixture = fixture();
    for config in ["tsconfig.json", "exact.json", "loose.json"] {
        let project = fixture.load_config(config);
        // Outer-parameter discovery resolves these symbols before choosing
        // the healthy sibling. Native cold controls observe TS2304 even when
        // the final root is healthy; that acquired recovery remains invalid.
        for (marker, reason) in [
            ("lazyHealthy", SemanticFailureReason::UnresolvedSymbol),
            ("missingHealthy", SemanticFailureReason::ErrorType),
            ("lazyFailure", SemanticFailureReason::UnresolvedSymbol),
            ("missingFailure", SemanticFailureReason::ErrorType),
        ] {
            let answer = project.type_of(request(&fixture, marker));
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{config}/{marker}: {answer:?}");
            };
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
            assert_eq!(error.reason, reason.into(), "{config}/{marker}: {error:?}");
            let (file, text, name) = if marker.starts_with("lazy") {
                (
                    "values.ts",
                    include_str!("../fixtures/types/native-type-query-parameters/values.ts"),
                    "missingExport",
                )
            } else {
                ("cases.ts", SOURCE, "missingValue")
            };
            assert_eq!(error.path.as_deref(), Some(fixture.path(file).as_path()));
            assert_eq!(
                error.position,
                Some(position(text, text.find(name).unwrap()))
            );
        }
    }
}

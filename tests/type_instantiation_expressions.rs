//! Native instantiation-expression mapping, filtering, preserved members and failures.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-instantiation-expressions/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-instantiation-expressions/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-instantiation-expressions/tsconfig.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-instantiation-expressions/exact.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-instantiation-expressions/loose.json"),
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
fn written_type_arguments_filter_and_instantiate_in_native_order() {
    check(&[
        ("runtime", "(value: number) => number"),
        ("call", "number"),
        ("defaults", "(first: string, second: string) => string"),
        ("empty", "(value: unknown) => unknown"),
        ("rejected", "<T extends string>(value: T) => T"),
        ("arity", "{}"),
        ("nongeneric", "{}"),
    ]);
}

#[test]
fn instantiation_preserves_properties_indexes_and_construct_signatures() {
    check(&[
        (
            "both",
            "{ (value: number): number; new (value: number): { value: number; }; readonly tag: \"tag\"; }",
        ),
        ("property", "\"tag\""),
        ("constructed", "number"),
        ("index", "string"),
    ]);
}

#[test]
fn constituent_mapping_preserves_native_union_intersection_and_scalar_results() {
    check(&[
        (
            "union",
            "((value: number) => number) | ((value: number) => number[])",
        ),
        ("intersection", "\"intersection\""),
        (
            "mixed",
            "{ readonly tag: \"mixed\"; } | ((value: number) => number)",
        ),
        ("any", "any"),
        ("never", "never"),
        ("literal", "\"literal\""),
    ]);
}

#[test]
fn outer_parameters_and_generic_constraints_reuse_the_actual_expression_node() {
    check(&[
        ("outer", "(value: number) => number"),
        ("constraint", "(value: number) => number"),
    ]);
}

#[test]
fn consumed_instantiation_dependencies_keep_the_original_failure() {
    let fixture = fixture();
    for config in ["tsconfig.json", "exact.json", "loose.json"] {
        let project = fixture.load_config(config);
        for marker in ["returnFailure", "argumentFailure", "operandFailure"] {
            let answer = project.type_of(request(&fixture, marker));
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{config}/{marker}: {answer:?}");
            };
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
            assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
            assert!(
                error.path.is_some() && error.position.is_some(),
                "{config}/{marker}: {error:?}"
            );
        }
    }
}

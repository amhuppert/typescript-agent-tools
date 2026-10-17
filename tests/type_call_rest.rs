//! Native effective arguments are shared by arity, inference and applicability.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const DEFINITIONS: &str = include_str!("../fixtures/types/native-call-rest/definitions.ts");
const SITES: &str = include_str!("../fixtures/types/native-call-rest/sites.ts");
const EXPECTED: &str = include_str!("../fixtures/types/native-call-rest/expected.json");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("definitions.ts", DEFINITIONS),
        ("sites.ts", SITES),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-call-rest/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-call-rest/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-call-rest/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-call-rest/exact.json"),
        ),
    ])
}
fn request(fixture: &Fixture, name: &str, expand: bool) -> TypeRequest {
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("sites.ts"),
            position: position(SITES, SITES.find(&format!("{name}Result =")).unwrap()),
        },
        expand,
        depth: 0,
    }
}
fn healthy(names: &[&str]) {
    let fixture = fixture();
    let expected: serde_json::Value = serde_json::from_str(EXPECTED).unwrap();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for &name in names {
            let summary = expected[name]
                .as_str()
                .unwrap_or_else(|| expected[name][config].as_str().unwrap());
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, name, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{name}: {result:?}"
                );
                assert_eq!(result.description.summary, summary, "{config}/{name}");
                assert!(
                    result.issues.is_empty(),
                    "{config}/{name}: {:?}",
                    result.issues
                );
            }
        }
    }
}
#[test]
fn fixed_optional_and_array_spreads_use_effective_arity() {
    healthy(&[
        "fixed",
        "spreadFixed",
        "readonlyFixed",
        "optionalOne",
        "optionalTwo",
        "optionalSpread",
        "arraySpread",
        "readonlyArraySpread",
    ]);
}
#[test]
fn inferred_rest_tuples_preserve_labels_flags_and_literal_context() {
    healthy(&[
        "empty",
        "scalar",
        "labels",
        "readonlyLabels",
        "optionalLabels",
        "restLabels",
        "multiple",
        "array",
        "emptySpread",
    ]);
}
#[test]
fn const_rest_inference_and_signature_parameter_slices_preserve_readonly_policy() {
    healthy(&[
        "constScalar",
        "constArray",
        "mutableConst",
        "callback",
        "constCallback",
    ]);
}
#[test]
fn tuple_inference_matches_prefixes_suffixes_and_constrained_variadic_slices() {
    healthy(&[
        "middle",
        "middleSpread",
        "optionalTail",
        "leading",
        "trailing",
        "parts",
        "tupleArgument",
        "tupleRest",
    ]);
}
#[test]
fn consumed_spread_and_arity_dependencies_stay_failed_across_healthy_requests() {
    let fixture = fixture();
    let expected_path = fixture.path("definitions.ts").canonicalize().unwrap();

    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for name in ["badSpread", "fixed", "aritySkipped", "badSpread"] {
            let dependency = if name == "aritySkipped" {
                "MissingUnusedArity"
            } else {
                "MissingRestDependency"
            };
            let expected_position = position(DEFINITIONS, DEFINITIONS.find(dependency).unwrap());
            let answer = project.type_of(request(&fixture, name, false));
            if name == "fixed" {
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{name}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(result.description.summary, "\"fixed\"");
                assert!(result.issues.is_empty());
            } else {
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error)) if
                    error.kind == SemanticErrorKind::InvalidSemantic
                    && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
                    && error.path.as_deref() == Some(expected_path.as_path())
                    && error.position == Some(expected_position)),
                    "{config}: {answer:?}"
                );
            }
        }
    }
}

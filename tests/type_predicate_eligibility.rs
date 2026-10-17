//! Final calls distinguish native predicate eligibility from excluded F output.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-predicate-eligibility/cases.ts");

fn check(marker: &str, strict_expected: &str, loose_expected: &str, flow_required: bool) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-predicate-eligibility/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-predicate-eligibility/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-predicate-eligibility/loose.json"),
        ),
    ]);
    for (config, expected) in [
        ("tsconfig.json", strict_expected),
        ("loose.json", loose_expected),
    ] {
        let project = fixture.load_config(config);
        for expand in [false, true] {
            let tag = format!("/*#{marker}*/");
            let mut request = TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
            });
            request.expand = expand;
            let answer = project.type_of(request);
            if flow_required {
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error))
                    if error.kind == SemanticErrorKind::UnsupportedSemantic
                    && error.reason == SemanticFailureReason::FlowRequired.into()),
                    "{config}/{marker}: {answer:?}"
                );
            } else {
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{marker}");
                assert_eq!(result.description.summary, expected, "{config}/{marker}");
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
fn nonboolean_body_does_not_enter_parameter_refinement() {
    check("nonBoolean", "\"other\"", "\"other\"", false);
}
#[test]
fn multiple_returns_stop_before_predicate_expression_checking() {
    check("multiple", "\"boolean\"", "\"boolean\"", false);
}
#[test]
fn bare_return_is_ineligible() {
    check("bare", "\"other\"", "\"other\"", false);
}
#[test]
fn implicit_return_uses_shared_reachability() {
    check("implicit", "\"other\"", "\"boolean\"", false);
}
#[test]
fn unchanged_parameter_flow_establishes_no_predicate() {
    check("unchanged", "\"boolean\"", "\"boolean\"", false);
}
#[test]
fn boolean_parameters_skip_refinement_before_flow() {
    check("booleanParameter", "\"other\"", "\"boolean\"", false);
}
#[test]
fn assigned_parameters_skip_refinement_before_flow() {
    check("assigned", "\"boolean\"", "\"boolean\"", false);
}
#[test]
fn rest_parameters_skip_refinement_after_assignment_observation() {
    check("rest", "\"boolean\"", "\"boolean\"", false);
}
#[test]
fn native_predicate_refinement_preserves_flow_required() {
    check("inferred", "\"predicate\"", "\"predicate\"", true);
}
#[test]
fn one_sided_refinement_stops_at_the_first_changed_flow_read() {
    check("oneSided", "\"boolean\"", "\"boolean\"", true);
}

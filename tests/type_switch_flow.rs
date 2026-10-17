//! Switch entry, fallthrough and exhaustiveness affect final reads and returns.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-switch-flow/flow.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("flow.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-switch-flow/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-switch-flow/loose.json"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str) -> TypeRequest {
    let token = format!("/*#{marker}*/ ");
    let offset = SOURCE.find(&token).unwrap() + token.len();
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("flow.ts"),
        position: position(SOURCE, offset),
    });
    request.expand = true;
    request.depth = 3;
    request
}

fn found(markers: &[(&str, &str)]) {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for &(marker, expected) in markers {
            let answer = project.type_of(request(&fixture, marker));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
                "native switch identity must be established ({config}, {marker}): {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.description.summary, expected, "{config}: {marker}");
            assert!(result.issues.is_empty());
            assert!(result.description.expansion.is_some());
        }
    }
}

#[test]
fn unrelated_switch_does_not_demand_erroneous_case_types() {
    found(&[
        ("unrelated-case", "number"),
        ("unrelated-default", "number"),
        ("unrelated-after", "number"),
    ]);
}

#[test]
fn typeof_witnesses_preserve_identity_and_native_group_fallthrough_order() {
    found(&[
        ("typeof-number", "number"),
        ("typeof-after", "number"),
        ("duplicate-first", "number"),
        ("host-default", "number"),
        ("grouped-types", "number"),
        ("default-grouped-number", "number"),
        ("fallthrough-number", "number"),
        ("fallthrough-after", "number"),
        ("escaped-witness-number", "number"),
        ("empty-witness-default", "number"),
        ("unpaired-witness-default", "number"),
    ]);
}

#[test]
fn one_nonliteral_disables_all_typeof_witnesses_without_demanding_its_type() {
    found(&[
        ("nil-string", "number"),
        ("nil-missing", "number"),
        ("nil-default", "number"),
        ("nil-after", "number"),
        ("parenthesized-witness-string", "number"),
        ("parenthesized-witness-number", "number"),
        ("parenthesized-witness-default", "number"),
    ]);
}

#[test]
fn matching_discriminant_default_and_join_preserve_nonunit_primitive_identity() {
    found(&[("direct-default", "number"), ("direct-after", "number")]);
}

#[test]
fn changed_switch_reads_require_flow_instead_of_returning_declared_types() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for marker in [
            "direct-case",
            "typeof-default",
            "duplicate-second",
            "duplicate-default",
            "host-case",
            "grouped-default",
            "default-later-string",
            "fallthrough-string",
            "template-witness-string",
            "escaped-witness-default",
            "empty-witness-case",
            "unpaired-witness",
        ] {
            let answer = project.type_of(request(&fixture, marker));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::UnsupportedSemantic
                && error.reason == SemanticFailureReason::FlowRequired.into()),
                "changed native switch identity consumes excluded narrowing ({config}, {marker}): {answer:?}"
            );
        }
    }
}

#[test]
fn exact_surrogate_case_values_keep_changed_and_unchanged_reads_distinct() {
    found(&[("unpaired-default", "string")]);
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        let request = request(&fixture, "unpaired-case");
        let TypeQuery::At { file, position } = &request.query else {
            unreachable!()
        };
        let (file, position) = (file.clone(), *position);
        let answer = project.type_of(request);
        let Err(AnalysisError::Semantic(error)) = answer else {
            panic!("{config}: {answer:?}");
        };
        assert_eq!(
            error.kind,
            SemanticErrorKind::UnsupportedSemantic,
            "{error:?}"
        );
        assert_eq!(
            error.reason,
            SemanticFailureReason::FlowRequired.into(),
            "{error:?}"
        );
        assert_eq!(error.path, Some(file));
        assert_eq!(error.position, Some(position));
        assert!(error.causes.is_empty(), "{error:?}");
    }
}

#[test]
fn exhaustive_switches_remove_only_the_actual_implicit_return_path() {
    found(&[
        ("exhaustive-function", "() => 1 | 2"),
        ("true-function", "() => 1 | 2"),
        (
            "all-typeof-function",
            "(value: any) => 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8",
        ),
    ]);
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, strict, loose) in [
            ("incomplete-function", "() => 1 | undefined", "() => number"),
            ("nil-function", "() => 1 | 2 | undefined", "() => 1 | 2"),
            (
                "incomplete-typeof-function",
                "(value: any) => 1 | 2 | 3 | 4 | 5 | 6 | 7 | undefined",
                "(value: any) => 1 | 2 | 3 | 4 | 5 | 6 | 7",
            ),
        ] {
            let answer = project.type_of(request(&fixture, marker));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
                "switch end reachability must be established ({config}, {marker}): {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(
                result.description.summary,
                if config == "tsconfig.json" {
                    strict
                } else {
                    loose
                }
            );
            assert!(result.issues.is_empty());
        }
    }
}

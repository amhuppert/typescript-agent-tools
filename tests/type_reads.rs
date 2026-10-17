//! Final read outcomes use native binder flow and restricted checker admission.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-read-admission/reads.ts");
const EXPORTS: &str = include_str!("../fixtures/types/native-read-admission/exports.ts");
const STRICT: &str = include_str!("../fixtures/types/native-read-admission/tsconfig.json");
const LOOSE: &str = include_str!("../fixtures/types/native-read-admission/loose.json");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("reads.ts", SOURCE),
        ("exports.ts", EXPORTS),
        ("tsconfig.json", STRICT),
        ("loose.json", LOOSE),
    ])
}

fn request(fixture: &Fixture, marker: &str) -> TypeRequest {
    let comment = format!("/*{marker}*/ ");
    let offset = SOURCE.find(&comment).unwrap() + comment.len();
    TypeRequest::new(TypeQuery::At {
        file: fixture.path("reads.ts"),
        position: position(SOURCE, offset),
    })
}

fn check_found(markers: &[(&str, &str)]) {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for &(marker, summary) in markers {
            let answer = project.type_of(request(&fixture, marker));
            assert!(
                matches!(answer, Ok(TypeAnswer::Selected(_))),
                "an admitted exact site read must succeed ({config}, {marker}): {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.target.kind, TypeQuestion::Expression);
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, summary, "{config}: {marker}");
            assert!(result.issues.is_empty());
        }
    }
}

#[test]
fn initialized_nonunion_and_ambient_union_reads_preserve_native_identity() {
    check_found(&[
        ("fixed-read", "1"),
        ("count-read", "number"),
        ("declared-read", "string | number"),
    ]);
}

#[test]
fn assignments_are_interpreted_for_the_actual_canonical_reference() {
    check_found(&[
        ("after-unrelated-assignment", "number"),
        ("after-own-assignment", "number"),
        ("shadowed-inner-read", "string"),
        ("after-shadowed-assignment", "number"),
        ("after-shadowed-condition", "number"),
    ]);
}

#[test]
fn irrelevant_and_constant_guards_and_unreachable_nodes_keep_exact_reads() {
    check_found(&[
        ("inside-unrelated-condition", "number"),
        ("inside-constant-condition", "number"),
        ("inside-unreachable-condition", "string | number"),
        ("inside-unreachable-loop", "number"),
        ("after-unreachable-loop", "number"),
    ]);
}

#[test]
fn function_boundaries_use_native_local_and_captured_read_setup() {
    check_found(&[
        ("local-start-read", "number"),
        ("outer-function-read", "number"),
    ]);
}

#[test]
fn a_never_antecedent_does_not_consume_condition_narrowing() {
    check_found(&[("never-condition-read", "never")]);
}

#[test]
fn raw_unknown_condition_and_call_symbols_retain_resolution_recovery() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, name) in [
            ("unknown-condition-read", "Missing"),
            ("after-unresolved-call", "MissingCall"),
            ("after-comma-ordinary-calls", "MissingCall"),
        ] {
            let answer = project.type_of(request(&fixture, marker));
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{config}/{marker}: {answer:?}")
            };
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
            assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
            assert!(error.message.contains(name));
            // Raw condition-symbol recovery has no type acquisition site;
            // the read consuming it supplies the location. Calls resolve at
            // their own callee before the later read.
            let offset = if marker == "unknown-condition-read" {
                SOURCE.find("/*unknown-condition-read*/ ").unwrap()
                    + "/*unknown-condition-read*/ ".len()
            } else {
                SOURCE.find("MissingCall()").unwrap()
            };
            assert_eq!(error.position, Some(position(SOURCE, offset)));
        }
    }
}

#[test]
fn definite_assignment_positions_return_the_native_declared_type() {
    check_found(&[
        ("definite-union-write", "string | number"),
        ("logical-union-write", "string | number"),
        ("numeric-compound-write", "number"),
    ]);
}

#[test]
fn required_definite_assignment_stays_a_flow_obligation() {
    let fixture = fixture();
    let strict = fixture.load_config("tsconfig.json");
    let answer = strict.type_of(request(&fixture, "uninitialized-number-read"));
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::UnsupportedSemantic && error.reason == SemanticFailureReason::FlowRequired.into()),
        "required definite-assignment analysis must stay explicit: {answer:?}"
    );
    let loose = fixture.load_config("loose.json");
    let answer = loose
        .type_of(request(&fixture, "uninitialized-number-read"))
        .unwrap();
    let TypeAnswer::Selected(result) = answer else {
        panic!("ordinary loose read must be selected")
    };
    assert_eq!(result.description.summary, "number");
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert!(result.issues.is_empty());
}

#[test]
fn direct_nonnull_context_is_a_distinct_native_initialization_question() {
    check_found(&[("nonnull-initialization-read", "number")]);
    let fixture = fixture();
    let project = fixture.load_config("tsconfig.json");
    let answer = project.type_of(request(&fixture, "parenthesized-nonnull-read"));
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::UnsupportedSemantic && error.reason == SemanticFailureReason::FlowRequired.into()),
        "a parenthesized operand is not a direct NonNull parent: {answer:?}"
    );
}

#[test]
fn alias_reads_keep_native_own_identity_and_initialization_context() {
    check_found(&[
        ("first-alias-read", "number"),
        ("second-alias-read", "number"),
        ("union-alias-read", "string | number"),
        ("function-alias-read", "() => void"),
        ("captured-alias-read", "number"),
        ("captured-alias-arrow-read", "number"),
        ("local-arrow-nonnull-read", "number"),
        ("other-alias-condition-read", "number"),
    ]);
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        let answer = project.type_of(request(&fixture, "same-alias-condition-read"));
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::UnsupportedSemantic && error.reason == SemanticFailureReason::FlowRequired.into()),
            "a matching raw alias guard still consumes excluded narrowing ({config}): {answer:?}"
        );
    }
}

#[test]
fn readonly_assignment_error_recovery_remains_invalid() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        let answer = project.type_of(request(&fixture, "constant-write"));
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::ErrorType.into()),
            "readonly assignment must preserve consumed native error type ({config}): {answer:?}"
        );
    }
}

#[test]
fn native_assignment_kinds_and_literal_base_conversion_are_distinct() {
    check_found(&[
        ("compound-plus", "number"),
        ("compound-minus", "number"),
        ("compound-multiply", "number"),
        ("compound-power", "number"),
        ("compound-divide", "number"),
        ("compound-remainder", "number"),
        ("compound-and", "number"),
        ("compound-or", "number"),
        ("compound-xor", "number"),
        ("compound-shift-left", "number"),
        ("compound-shift-right", "number"),
        ("compound-shift-unsigned", "number"),
        ("compound-postfix", "number"),
        ("compound-prefix", "number"),
        ("literal-definite", "1"),
        ("literal-compound", "number"),
        ("literal-compound-like", "number"),
        ("literal-bitwise-rhs", "1"),
        ("literal-union-definite", "1 | 2"),
        ("literal-union-compound-like", "number"),
        ("logical-and-write", "string | number"),
        ("logical-nullish-write", "string | number"),
        ("for-in-write", "string | number"),
        ("for-of-write", "string | number"),
    ]);
}

#[test]
fn matching_flow_assignments_consume_native_compound_base_questions() {
    check_found(&[
        ("after-literal-compound", "number"),
        ("after-literal-compound-like", "number"),
        ("after-ambient-union-compound", "string | number"),
    ]);
}

#[test]
fn consumed_union_assignment_and_guard_refinement_require_flow() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for marker in ["refined-read", "flag-read", "inside-own-condition"] {
            let answer = project.type_of(request(&fixture, marker));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                    if error.kind == SemanticErrorKind::UnsupportedSemantic
                    && error.reason == SemanticFailureReason::FlowRequired.into()),
                "a consumed excluded flow operation must stay explicit ({config}, {marker}): {answer:?}"
            );
            let Err(AnalysisError::Semantic(error)) = answer else {
                unreachable!()
            };
            let TypeQuery::At { file, position } = request(&fixture, marker).query else {
                unreachable!()
            };
            assert_eq!(error.path, Some(file));
            assert_eq!(error.position, Some(position));
        }
    }
}

#[test]
fn automatic_read_admission_uses_retained_implicit_any_options() {
    let fixture = fixture();
    let strict = fixture.load_config("tsconfig.json");
    let answer = strict.type_of(request(&fixture, "automatic-read"));
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error))
            if error.kind == SemanticErrorKind::UnsupportedSemantic
            && error.reason == SemanticFailureReason::FlowRequired.into()),
        "automatic use-site evolution must remain a flow obligation: {answer:?}"
    );
    let loose = fixture.load_config("loose.json");
    let answer = loose.type_of(request(&fixture, "automatic-read")).unwrap();
    let TypeAnswer::Selected(result) = answer else {
        panic!("the nonautomatic read is selected")
    };
    assert_eq!(result.target.kind, TypeQuestion::Expression);
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert_eq!(result.description.summary, "any");
    assert!(result.issues.is_empty());
}

#[test]
fn every_iterative_statement_uses_native_loop_entry_identity() {
    check_found(&[
        ("inside-while-identity", "number"),
        ("after-while-identity", "number"),
        ("inside-do-identity", "number"),
        ("after-do-identity", "number"),
        ("inside-for-unreachable", "number"),
        ("after-for-identity", "number"),
        ("inside-for-in-identity", "number"),
        ("after-for-in-identity", "number"),
        ("inside-for-of-identity", "number"),
        ("after-for-of-identity", "number"),
        ("inside-cyclic-identity", "number"),
        ("after-infinite-loop", "number"),
    ]);
}

#[test]
fn labelled_targets_preserve_identity_and_native_boundary_rules() {
    check_found(&[
        ("before-labelled-break", "number"),
        ("after-labelled-break-unreachable", "number"),
        ("after-labelled-break", "number"),
        ("before-labelled-continue", "number"),
        ("after-labelled-continue-unreachable", "number"),
        ("after-labelled-infinite-loop", "number"),
        ("inside-labelled-block", "number"),
        ("after-block-break-unreachable", "number"),
        ("after-labelled-block", "number"),
        ("before-escaped-label-break", "number"),
        ("after-escaped-label", "number"),
    ]);
}

#[test]
fn established_no_effect_calls_keep_exact_reads_without_argument_or_body_warmup() {
    check_found(&[
        ("after-ordinary-call", "number"),
        ("after-generic-ordinary-call", "number"),
        ("after-annotated-call", "number"),
        ("after-overload-ordinary-call", "number"),
        ("after-untyped-call", "number"),
    ]);
}

#[test]
fn ordinary_predicates_and_never_calls_keep_distinct_native_flow_results() {
    check_found(&[
        ("after-nonasserting-predicate-call", "number"),
        ("after-never-call", "number"),
    ]);
}

#[test]
fn assertion_false_expressions_stop_reachability_before_assignment_reduction() {
    check_found(&[
        ("after-false-assertion-assignment", "string | number"),
        ("after-parenthesized-false-assertion", "string | number"),
        ("after-and-left-false-assertion", "string | number"),
        ("after-and-right-false-assertion", "string | number"),
        ("after-or-false-assertion", "string | number"),
    ]);
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for marker in ["after-true-assertion-assignment", "after-true-or-assertion"] {
            let answer = project.type_of(request(&fixture, marker));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::UnsupportedSemantic
                && error.reason == SemanticFailureReason::FlowRequired.into()),
                "a reachable assignment still consumes actual excluded reduction ({config}, {marker}): {answer:?}"
            );
        }
    }
}

#[test]
fn logical_and_nullish_branches_use_native_condition_targets_and_effect_restoration() {
    check_found(&[
        ("after-pure-logical-and", "number"),
        ("after-pure-logical-or", "number"),
        ("after-pure-logical-nullish", "number"),
        ("after-negated-pure-logical", "number"),
        ("after-logical-and-effects", "number"),
        ("after-logical-or-effects", "number"),
        ("after-logical-nullish-effects", "number"),
        ("inside-nested-logical-condition", "number"),
        ("inside-negated-logical-condition", "number"),
    ]);
}

#[test]
fn assertion_truthiness_preserves_identity_and_consumes_the_incoming_flow_first() {
    check_found(&[
        ("inside-matching-number-truthy", "number"),
        ("inside-matching-number-falsy", "number"),
        ("after-matching-number-assertion", "number"),
        ("after-number-or-false-assertion", "number"),
        ("after-false-or-number-assertion", "number"),
        ("after-number-and-true-assertion", "number"),
        ("after-zero-assertion", "number"),
        ("after-null-assertion", "number"),
        ("after-negated-true-assertion", "number"),
        ("inside-matching-literal-true", "true"),
        ("after-direct-false-assertion", "number"),
        ("after-direct-and-false-assertion", "number"),
        (
            "false-assertion-consumed-union-antecedent",
            "string | number",
        ),
    ]);
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        if config == "loose.json" {
            let answer = project
                .type_of(request(&fixture, "inside-matching-literal-false"))
                .unwrap();
            let TypeAnswer::Selected(result) = answer else {
                unreachable!()
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, "true");
            assert!(result.issues.is_empty());
        }
        let markers = if config == "tsconfig.json" {
            &[
                "inside-matching-literal-false",
                "after-direct-or-false-assertion",
            ][..]
        } else {
            &["after-direct-or-false-assertion"][..]
        };
        for &marker in markers {
            let answer = project.type_of(request(&fixture, marker));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::UnsupportedSemantic
                    && error.reason == SemanticFailureReason::FlowRequired.into()),
                "a consumed flow refinement must remain a failure ({config}, {marker}): {answer:?}"
            );
        }
    }
}

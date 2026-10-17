//! Native exception and finally reductions affect final reads and returns.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-try-flow/flow.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("flow.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-try-flow/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-try-flow/loose.json"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str) -> TypeRequest {
    let comment = format!("/*{marker}*/ ");
    let offset = SOURCE.find(&comment).unwrap() + comment.len();
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
        for &(marker, summary) in markers {
            let answer = project.type_of(request(&fixture, marker));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
                "native try/finally path ({config}, {marker}) must be established: {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.description.summary, summary, "{config}: {marker}");
            assert!(result.issues.is_empty());
            assert!(result.description.expansion.is_some());
        }
    }
}

#[test]
fn unrelated_mutations_preserve_reads_in_try_catch_and_nested_finally_paths() {
    found(&[
        ("try-read", "number"),
        ("catch-read", "number"),
        ("after-catch", "number"),
        ("finally-read", "number"),
        ("after-finally", "number"),
        ("nested-catch", "number"),
        ("after-nested-finally", "number"),
    ]);
}

#[test]
fn exception_entry_precedes_mutations_but_normal_exit_keeps_consumed_refinement() {
    // The normal path's assignment refines `changed` to number, then joins
    // the catch path's declared type. The completed read equals the declared
    // type, so admission returns the pinned compiler's answer rather than
    // refusing an intermediate refinement (types-design.md, flow admission).
    found(&[
        ("exception-first-entry", "string | number"),
        ("normal-path-refinement", "string | number"),
    ]);
}

#[test]
fn calls_are_separate_from_exception_mutations_and_failed_effects_stay_failures() {
    found(&[("catch-before-call-effect", "number")]);
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let answer = fixture
            .load_config(config)
            .type_of(request(&fixture, "normal-invalid-effect"));
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error))
            if error.kind == SemanticErrorKind::InvalidSemantic
            && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
            "normal exit consumes its missing assertion metadata ({config}): {answer:?}"
        );
    }
}

#[test]
fn finally_end_reachability_controls_actual_inferred_return_types() {
    found(&[
        ("try-return", "() => number"),
        ("try-throw", "() => void"),
        ("try-catch-return", "() => number"),
        ("finally-return", "() => number"),
        ("try-never-catch", "() => void"),
        ("try-never-finally", "() => void"),
    ]);
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let answer = fixture
            .load_config(config)
            .type_of(request(&fixture, "finally-conditional-return"));
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
            "finalizer return and normal reduction must both be established ({config}): {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(
            result.description.summary,
            if config == "tsconfig.json" {
                "() => 1 | 2 | undefined"
            } else {
                "() => 1 | 2"
            }
        );
        assert!(result.issues.is_empty());
    }
}

#[test]
fn unreachable_syntax_does_not_create_exception_or_call_effect_paths() {
    found(&[
        ("unreachable-call-after-return", "() => number"),
        ("unreachable-try-after-return", "() => number"),
        ("unreachable-read-after-try", "number"),
    ]);
}

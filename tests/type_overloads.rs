//! Native candidate order and relation passes decide the final call result.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::project::Project;
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SITES: &str = include_str!("../fixtures/types/native-overloads/sites.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-overloads/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-overloads/loose.json"),
        ),
        (
            "reverse.json",
            include_str!("../fixtures/types/native-overloads/reverse.json"),
        ),
        (
            "reverse-loose.json",
            include_str!("../fixtures/types/native-overloads/reverse-loose.json"),
        ),
        (
            "definitions.ts",
            include_str!("../fixtures/types/native-overloads/definitions.ts"),
        ),
        (
            "globals-first.ts",
            include_str!("../fixtures/types/native-overloads/globals-first.ts"),
        ),
        (
            "globals-second.ts",
            include_str!("../fixtures/types/native-overloads/globals-second.ts"),
        ),
        ("sites.ts", SITES),
    ])
}

fn answer(project: &Project, fixture: &Fixture, name: &str) -> Result<TypeAnswer, AnalysisError> {
    let offset = SITES.find(&format!("{name} =")).unwrap();
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("sites.ts"),
        position: position(SITES, offset),
    });
    request.expand = true;
    request.depth = 2;
    project.type_of(request)
}

fn found(project: &Project, fixture: &Fixture, name: &str, expected: &str) {
    let answer = answer(project, fixture, name);
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
        "{name} must select its actual native overload: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(
        result.description.summary,
        format!("\"{expected}\""),
        "{name}"
    );
    assert!(result.issues.is_empty(), "{name}");
    let graph = result.description.expansion.unwrap();
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    assert!(
        matches!(&root.data, TypeNodeData::Literal { value, .. } if value == expected),
        "{name}: {root:?}"
    );
}

fn failed(
    project: &Project,
    fixture: &Fixture,
    name: &str,
    reason: SemanticFailureReason,
    token: &str,
) {
    let start = SITES.find(&format!("{name} =")).unwrap();
    let expected = position(SITES, start + SITES[start..].find(token).unwrap());
    let answer = answer(project, fixture, name);
    let path = fixture.path("sites.ts").canonicalize().unwrap();
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error))
            if error.kind == SemanticErrorKind::InvalidSemantic
                && error.reason == reason.into()
                && error.path.as_deref() == Some(path.as_path())
                && error.position == Some(expected)),
        "{name} must keep the actual visited failure and location: {answer:?}"
    );
}

#[test]
fn literal_specialization_and_subtype_before_assignable_choose_exact_native_returns() {
    let fixture = fixture();
    for config in [
        "tsconfig.json",
        "loose.json",
        "reverse.json",
        "reverse-loose.json",
    ] {
        let project = fixture.load_config(config);
        for (name, expected) in [
            ("exactResult", "exact"),
            ("escapedResult", "exact"),
            ("broadResult", "broad"),
            ("numberResult", "number"),
            ("trueResult", "yes"),
            ("anyResult", "any"),
            ("typedResult", "number"),
            ("tieResult", "first"),
            ("callableResult", "exact-call"),
        ] {
            found(&project, &fixture, name, expected);
        }
    }
}

#[test]
fn actual_declaration_parent_groups_reverse_global_priority_with_file_order() {
    let fixture = fixture();
    for (config, expected) in [
        ("tsconfig.json", "second-file"),
        ("loose.json", "second-file"),
        ("reverse.json", "first-file"),
        ("reverse-loose.json", "first-file"),
    ] {
        found(
            &fixture.load_config(config),
            &fixture,
            "groupedResult",
            expected,
        );
    }
}

#[test]
fn arity_and_this_rejections_continue_to_a_later_eligible_candidate() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (name, expected) in [
            ("oneResult", "one"),
            ("twoResult", "two"),
            ("omittedResult", "void"),
            ("thisResult", "void"),
        ] {
            found(&project, &fixture, name, expected);
        }
    }
}

#[test]
fn candidate_recovery_does_not_turn_the_native_fallback_return_into_a_healthy_answer() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (name, token) in [
            ("failedResult", "choose(false)"),
            ("wrongArityResult", "arity(1, 2, 3)"),
            ("extraMissingResult", "arity(1, 2, Missing)"),
            ("typeArgumentResult", "choose<number>(1)"),
        ] {
            failed(
                &project,
                &fixture,
                name,
                SemanticFailureReason::OverloadRecovery,
                token,
            );
        }
    }
}

#[test]
fn actually_checked_arguments_and_type_arguments_keep_their_original_missing_symbol() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        failed(
            &project,
            &fixture,
            "missingResult",
            SemanticFailureReason::UnresolvedSymbol,
            "Missing",
        );
        failed(
            &project,
            &fixture,
            "missingTypeArgumentResult",
            SemanticFailureReason::UnresolvedSymbol,
            "MissingType",
        );
    }
}

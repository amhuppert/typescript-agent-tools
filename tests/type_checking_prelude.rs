//! Actual signature checking precedes the selected site's own type question.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::project::Project;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest};

const DEFINITIONS: &str = include_str!("../fixtures/types/checking-prelude/definitions.ts");
const SITES: &str = include_str!("../fixtures/types/checking-prelude/sites.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/checking-prelude/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/checking-prelude/loose.json"),
        ),
        ("definitions.ts", DEFINITIONS),
        ("sites.ts", SITES),
    ])
}

fn offset(marker: &str) -> usize {
    let marker = format!("/*#{marker}*/");
    SITES.find(&marker).unwrap() + marker.len()
}

fn answer(project: &Project, fixture: &Fixture, marker: &str) -> Result<TypeAnswer, AnalysisError> {
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("sites.ts"),
        position: position(SITES, offset(marker)),
    });
    request.expand = true;
    request.depth = 3;
    project.type_of(request)
}

fn invalid(
    project: &Project,
    fixture: &Fixture,
    marker: &str,
    reason: SemanticFailureReason,
    file: &str,
    source: &str,
    cause: &str,
) {
    let answer = answer(project, fixture, marker);
    let path = fixture.path(file).canonicalize().unwrap();
    let expected = position(source, source.find(cause).unwrap());
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error))
        if error.kind == SemanticErrorKind::InvalidSemantic
            && error.reason == reason.into()
            && error.path.as_deref() == Some(path.as_path())
            && error.position == Some(expected)),
        "{marker} must preserve the exact consumed prelude dependency: {answer:?}"
    );
}

#[test]
fn enclosing_signatures_establish_before_the_literal_without_demanding_unused_returns() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [("ordinary", "1"), ("nested", "2"), ("returnHidden", "3")] {
            let answer = answer(&project, &fixture, marker);
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(result))
                if result.outcome == TypeOutcome::Found),
                "{marker} must type its own literal after native signature checking: {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.target.kind, TypeQuestion::Expression);
            assert_eq!(result.target.location.file, "sites.ts");
            assert_eq!(
                result.target.location.start,
                position(SITES, offset(marker))
            );
            assert_eq!(
                result.target.location.end,
                position(SITES, offset(marker) + 1)
            );
            assert_eq!(result.description.summary, summary);
            assert!(result.issues.is_empty());
            let graph = result.description.expansion.as_ref().unwrap();
            assert_eq!(
                graph.nodes.len(),
                1,
                "literal expansion must retain its established root"
            );
        }
    }
}

#[test]
fn outer_arity_this_and_argument_recovery_stop_before_typing_the_selected_literal() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, call) in [
            ("rejected", "take(/*#rejected*/"),
            ("outerRejected", "outer(inner(/*#outerRejected*/"),
            ("thisRejected", "wrongThis("),
        ] {
            invalid(
                &project,
                &fixture,
                marker,
                SemanticFailureReason::OverloadRecovery,
                "sites.ts",
                SITES,
                call,
            );
        }
    }
}

#[test]
fn parameter_recovery_is_consumed_but_an_undemanded_return_stays_healthy() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        invalid(
            &project,
            &fixture,
            "missingParameter",
            SemanticFailureReason::UnresolvedSymbol,
            "definitions.ts",
            DEFINITIONS,
            "MissingParameter",
        );
        let request = TypeRequest::new(TypeQuery::At {
            file: fixture.path("definitions.ts"),
            position: position(DEFINITIONS, DEFINITIONS.find("hiddenReturn").unwrap()),
        });
        let answer = project.type_of(request);
        let path = fixture.path("definitions.ts").canonicalize().unwrap();
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error))
            if error.kind == SemanticErrorKind::InvalidSemantic
                && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
                && error.path.as_deref() == Some(path.as_path())
                && error.position == Some(position(DEFINITIONS, DEFINITIONS.find("MissingReturn").unwrap()))),
            "a demanded signature return retains its own recovery: {answer:?}"
        );
    }
}

#[test]
fn selected_nulls_keep_the_preludes_native_option_dependent_applicability() {
    let fixture = fixture();
    invalid(
        &fixture.load(),
        &fixture,
        "nullable",
        SemanticFailureReason::OverloadRecovery,
        "sites.ts",
        SITES,
        "take(/*#nullable*/",
    );
    let answer = answer(&fixture.load_config("loose.json"), &fixture, "nullable");
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(result))
        if result.outcome == TypeOutcome::Found && result.description.summary == "null"),
        "loose null applicability must permit the selected literal: {answer:?}"
    );
}

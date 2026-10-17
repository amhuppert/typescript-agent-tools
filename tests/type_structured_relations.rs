//! Compiler-pinned structural applicability preserves actual demand order.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::project::Project;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest,
};

const SITES: &str = include_str!("../fixtures/types/native-structured-relations/sites.ts");
const DEFINITIONS: &str =
    include_str!("../fixtures/types/native-structured-relations/definitions.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-structured-relations/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-structured-relations/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-structured-relations/exact.json"),
        ),
        (
            "globals.ts",
            include_str!("../fixtures/types/native-structured-relations/globals.ts"),
        ),
        (
            "augmented.json",
            include_str!("../fixtures/types/native-structured-relations/augmented.json"),
        ),
        (
            "augmented-loose.json",
            include_str!("../fixtures/types/native-structured-relations/augmented-loose.json"),
        ),
        (
            "augmented-exact.json",
            include_str!("../fixtures/types/native-structured-relations/augmented-exact.json"),
        ),
        ("definitions.ts", DEFINITIONS),
        ("sites.ts", SITES),
    ])
}

fn answer(project: &Project, fixture: &Fixture, name: &str) -> Result<TypeAnswer, AnalysisError> {
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("sites.ts"),
        position: position(SITES, SITES.find(&format!("{name} =")).unwrap()),
    });
    request.expand = true;
    request.depth = 2;
    project.type_of(request)
}

fn found(project: &Project, fixture: &Fixture, name: &str, value: &str) {
    let answer = answer(project, fixture, name);
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
        "{name} must complete its actual structured comparison: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.description.summary, format!("\"{value}\""), "{name}");
    assert!(result.issues.is_empty(), "{name}");
    let graph = result.description.expansion.unwrap();
    let root = graph
        .nodes
        .into_iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    assert_eq!(
        root.data,
        TypeNodeData::Literal {
            base: PrimitiveName::String,
            value: value.into(),
        },
        "{name}"
    );
}

#[test]
fn target_unknown_any_and_readonly_skip_unresolved_source_property_reads() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for name in ["unknownResult", "anyResult", "readonlyResult"] {
            found(&project, &fixture, name, "chosen");
        }
    }
}

#[test]
fn ordinary_false_unmatched_and_identity_keep_unvisited_properties_out_of_later_overloads() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for name in ["mismatchFirstResult", "validIndexResult"] {
            found(&project, &fixture, name, "same");
        }
        found(&project, &fixture, "identityResult", "identity");
    }
    // Native getUnmatchedProperties asks getPropertyOfType, including actual
    // global Object augmentation. A valid global avoids consuming recovery.
    for config in [
        "augmented.json",
        "augmented-loose.json",
        "augmented-exact.json",
    ] {
        let project = fixture.load_config(config);
        found(&project, &fixture, "unmatchedResult", "same");
    }
}

#[test]
fn optional_property_type_skip_and_required_rejection_preserve_profile_specific_operations() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        found(&project, &fixture, "optionalUnknownResult", "chosen");
        found(&project, &fixture, "optionalRequiredResult", "same");
    }
}

#[test]
fn visited_missing_property_and_inventory_index_types_preserve_their_original_invalidity() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (name, token) in [
            ("numberResult", "MissingSource;"),
            ("mismatchLastResult", "MissingSource;"),
            ("sourceIndexResult", "MissingSourceIndex;"),
            ("targetIndexResult", "MissingTargetIndex;"),
        ] {
            let answer = answer(&project, &fixture, name);
            let path = fixture.path("definitions.ts").canonicalize().unwrap();
            let expected = position(DEFINITIONS, DEFINITIONS.find(token).unwrap());
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::InvalidSemantic
                    && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
                    && error.path.as_deref() == Some(path.as_path())
                    && error.position == Some(expected)),
                "{name} must preserve the native visited child at its own declaration: {answer:?}"
            );
        }
    }
}

fn limit_fixture(input: &str, strict: bool) -> Fixture {
    let config = format!(
        "{{\"compilerOptions\":{{\"strict\":true,\"strictNullChecks\":{strict},\"noLib\":true}},\"files\":[\"sites.ts\"]}}"
    );
    Fixture::new(&[("tsconfig.json", &config), ("sites.ts", input)])
}

fn limit_answer(
    project: &Project,
    fixture: &Fixture,
    input: &str,
) -> Result<TypeAnswer, AnalysisError> {
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("sites.ts"),
        position: position(input, input.find("selected =").unwrap()),
    });
    request.expand = true;
    request.depth = 2;
    project.type_of(request)
}

fn found_limit(project: &Project, fixture: &Fixture, input: &str, value: &str) {
    let answer = limit_answer(project, fixture, input);
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
        "actual recursive call must complete: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.description.summary, format!("\"{value}\""));
    assert!(result.issues.is_empty());
    let graph = result.description.expansion.unwrap();
    let root = graph
        .nodes
        .into_iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    assert_eq!(
        root.data,
        TypeNodeData::Literal {
            base: PrimitiveName::String,
            value: value.into(),
        }
    );
}

#[test]
fn recursive_active_maybe_and_ordinary_false_select_the_exact_native_overload() {
    for strict in [true, false] {
        for (input, value) in [
            (
                include_str!("../fixtures/types/native-structured-relations/limits/activeMaybe.ts"),
                "chosen",
            ),
            (
                include_str!("../fixtures/types/native-structured-relations/limits/activeFalse.ts"),
                "same",
            ),
        ] {
            let fixture = limit_fixture(input, strict);
            let project = fixture.load_config("tsconfig.json");
            found_limit(&project, &fixture, input, value);
        }
    }
}

#[test]
fn consumed_relation_depth_guard_preserves_failure_at_and_above_the_native_boundary() {
    for strict in [true, false] {
        for (input, overflow) in [
            (
                include_str!("../fixtures/types/native-structured-relations/limits/depth99.ts"),
                false,
            ),
            (
                include_str!("../fixtures/types/native-structured-relations/limits/depth100.ts"),
                true,
            ),
            (
                include_str!("../fixtures/types/native-structured-relations/limits/depth101.ts"),
                true,
            ),
        ] {
            let fixture = limit_fixture(input, strict);
            let project = fixture.load_config("tsconfig.json");
            if !overflow {
                found_limit(&project, &fixture, input, "chosen");
                continue;
            }
            let answer = limit_answer(&project, &fixture, input);
            let path = fixture.path("sites.ts").canonicalize().unwrap();
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::InvalidSemantic
                    && error.reason == SemanticFailureReason::RelationDepthLimit.into()
                    && error.path.as_deref() == Some(path.as_path())),
                "visited native overflow must remain a failure despite the later healthy overload: {answer:?}"
            );
        }
    }
}

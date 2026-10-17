//! Native loop joins preserve incomplete paths until the completed question.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-loop-cache/loops.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("loops.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-loop-cache/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-loop-cache/loose.json"),
        ),
    ])
}

fn answer(fixture: &Fixture, config: &str, marker: &str) -> Result<TypeAnswer, AnalysisError> {
    let tag = format!("/*#{marker}*/");
    let offset = SOURCE.find(&tag).unwrap() + tag.len();
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("loops.ts"),
        position: position(SOURCE, offset),
    });
    request.expand = true;
    request.depth = 2;
    fixture.load_config(config).type_of(request)
}

fn full_union(fixture: &Fixture, config: &str, marker: &str) {
    let answer = answer(fixture, config, marker);
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found),
        "a completed native loop join must establish its full identity ({config}, {marker}): {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(
        result.description.summary, "string | number",
        "{config}: {marker}"
    );
    assert!(result.issues.is_empty());
    let graph = result.description.expansion.unwrap();
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    assert!(
        matches!(&root.data, TypeNodeData::Union { .. }),
        "{config}: {marker}"
    );
    assert_eq!(graph.nodes.len(), 3, "{config}: {marker}");
}

#[test]
fn nested_backedges_and_conditional_continues_restore_the_native_union() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        for marker in [
            "nestedAfter",
            "conditionalAfter",
            "continuesAfter",
            "innerHead",
            "innerExit",
            "innerEntryAfter",
            "backedgeInnerHead",
            "outerBackedgeAfter",
        ] {
            full_union(&fixture, config, marker);
        }
    }
}

#[test]
fn actual_assignment_rhs_reads_preserve_loop_reference_and_cache_identity() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        for marker in [
            "assignmentRhs",
            "referenceAssignmentAfter",
            "nestedRhs",
            "nestedReferenceAfter",
        ] {
            full_union(&fixture, config, marker);
        }
    }
}

#[test]
fn declared_entry_skips_the_invalid_backedge_and_unreachable_read_keeps_its_native_type() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        for marker in ["declaredEntryAfter", "unreachableAfter"] {
            full_union(&fixture, config, marker);
        }
    }
}

#[test]
fn actual_narrowed_loop_results_and_shadowed_origins_remain_flow_obligations() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        for marker in ["narrowedAfter", "doAfter", "shadowInner", "shadowOuter"] {
            let answer = answer(&fixture, config, marker);
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::UnsupportedSemantic
                    && error.reason == SemanticFailureReason::FlowRequired.into()),
                "an actual changed native read requires F ({config}, {marker}): {answer:?}"
            );
        }
    }
}

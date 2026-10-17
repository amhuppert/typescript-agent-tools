//! Ordinary inferred calls consume the shared slot/context owners end to end.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest,
};

const DEFINITIONS: &str = include_str!("../fixtures/types/native-call-inference/definitions.ts");
const SITES: &str = include_str!("../fixtures/types/native-call-inference/sites.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-call-inference/tsconfig.json"),
        ),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-call-inference/globals.d.ts"),
        ),
        ("definitions.ts", DEFINITIONS),
        ("sites.ts", SITES),
    ])
}

fn request(fixture: &Fixture, name: &str) -> TypeRequest {
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("sites.ts"),
        position: position(SITES, SITES.find(&format!("{name}Result =")).unwrap()),
    });
    request.expand = true;
    request.depth = 2;
    request
}

fn selected(name: &str, summary: &str) -> TypeNodeData {
    let fixture = fixture();
    let answer = fixture
        .load_config("tsconfig.json")
        .type_of(request(&fixture, name));
    let Ok(TypeAnswer::Selected(result)) = answer else {
        panic!("{name}: {answer:?}");
    };
    assert_eq!(result.outcome, TypeOutcome::Found, "{name}: {result:?}");
    assert!(result.issues.is_empty(), "{name}: {result:?}");
    assert_eq!(result.description.summary, summary, "{name}");
    let graph = result.description.expansion.unwrap();
    graph
        .nodes
        .into_iter()
        .find(|node| node.id == graph.root)
        .unwrap()
        .data
}

fn literal(name: &str, value: &str) {
    assert_eq!(
        selected(name, &format!("\"{value}\"")),
        TypeNodeData::Literal {
            base: PrimitiveName::String,
            value: value.into()
        }
    );
}

#[test]
fn scalar_candidates_preserve_return_literal() {
    literal("identity", "literal");
}

#[test]
fn ordered_scalar_candidates_form_the_native_union() {
    selected("pair", "\"a\" | \"b\"");
}

#[test]
fn nested_call_infers_from_its_active_return_context() {
    literal("context", "context");
}

#[test]
fn constraint_rejection_allows_the_later_overload() {
    literal("rejected", "number");
}

#[test]
fn eager_default_context_probe_preserves_original_failure() {
    let fixture = fixture();
    let answer = fixture
        .load_config("tsconfig.json")
        .type_of(request(&fixture, "eagerDefault"));
    let path = fixture.path("definitions.ts").canonicalize().unwrap();
    let expected = position(
        DEFINITIONS,
        DEFINITIONS.find("MissingEagerDefault").unwrap(),
    );
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error)) if
        error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into()
        && error.path.as_deref() == Some(path.as_path()) && error.position == Some(expected)),
        "{answer:?}"
    );
}

#[test]
fn inferred_slots_skip_defaults_after_the_first_absent_default() {
    literal("skippedDefault", "skipped");
}

#[test]
fn actual_access_receiver_supplies_generic_this_inference() {
    selected("receiver", "Receiver");
}

#[test]
fn omitted_optional_argument_does_not_demand_its_annotation() {
    selected("unused", "unknown");
}

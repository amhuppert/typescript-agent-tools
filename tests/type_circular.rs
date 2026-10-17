//! Native alias resolution distinguishes invalid cycles from deferred objects.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/circular-aliases/types.ts");
const STRICT: &str = include_str!("../fixtures/types/circular-aliases/tsconfig.json");
const LOOSE: &str = include_str!("../fixtures/types/circular-aliases/loose.json");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("types.ts", SOURCE),
        ("tsconfig.json", STRICT),
        ("loose.json", LOOSE),
    ])
}

fn request(fixture: &Fixture, name: &str) -> TypeRequest {
    let keyword = if matches!(name, "broken" | "healthy") {
        "let"
    } else {
        "type"
    };
    let marker = format!("{keyword} {name}");
    let offset = SOURCE.find(&marker).unwrap() + marker.find(name).unwrap();
    TypeRequest::new(TypeQuery::At {
        file: fixture.path("types.ts"),
        position: position(SOURCE, offset),
    })
}

fn check_invalid(names: &[(&str, &str)]) {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for &(selected, cause) in names {
            for expand in [false, true] {
                let mut request = request(&fixture, selected);
                request.expand = expand;
                request.depth = 0;
                let answer = project.type_of(request);
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error))
                        if error.kind == SemanticErrorKind::InvalidSemantic
                        && error.reason == SemanticFailureReason::CircularType.into()),
                    "consumed alias recovery must be invalid ({config}, {selected}, expand={expand}): {answer:?}"
                );
                let Err(AnalysisError::Semantic(error)) = answer else {
                    unreachable!()
                };
                let marker = format!("type {cause}");
                let offset = SOURCE.find(&marker).unwrap() + marker.find(cause).unwrap();
                assert_eq!(error.path, Some(fixture.path("types.ts")));
                assert_eq!(error.position, Some(position(SOURCE, offset)));
            }
        }
    }
}

#[test]
fn self_alias_cycles_fail_at_the_actual_alias_declaration() {
    check_invalid(&[("Loop", "Loop")]);
}

#[test]
fn mutual_alias_cycles_mark_each_participant_in_the_selected_resolution() {
    check_invalid(&[("First", "First"), ("Second", "Second")]);
}

#[test]
fn outer_alias_and_value_demands_preserve_the_consumed_cycle_cause() {
    check_invalid(&[("Outer", "First"), ("broken", "Loop")]);
}

#[test]
fn deferred_recursive_alias_members_reuse_the_established_root() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (name, depth) in ["Healthy", "healthy"]
            .into_iter()
            .flat_map(|name| [0, 1, 3, 32].map(|depth| (name, depth)))
        {
            let mut request = request(&fixture, name);
            request.expand = true;
            request.depth = depth;
            let answer = project.type_of(request).unwrap();
            let TypeAnswer::Selected(result) = answer else {
                panic!("the recursive alias is selected")
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, "Healthy");
            assert!(result.issues.is_empty());
            let graph = result.description.expansion.unwrap();
            if depth == 0 {
                assert_eq!(graph.nodes.len(), 1);
                assert!(graph.truncated);
            } else {
                assert_eq!(graph.nodes.len(), 2);
                assert!(!graph.truncated);
                let TypeNodeData::Object { members } = &graph.nodes[0].data else {
                    panic!("the recursive alias retains its object structure")
                };
                assert_eq!(
                    members.properties.as_ref().unwrap()[0].read_type,
                    TypeRef::Ref { r#ref: "t0".into() }
                );
            }
        }
    }
}

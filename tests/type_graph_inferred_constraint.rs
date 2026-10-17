mod support;
use support::{Fixture, position};
use typescript_parser::analysis::SemanticFailureReason;
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/graph-inferred-constraint/cases.ts");

#[test]
fn inferred_constraint_preserves_one_original_property_failure() {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/graph-inferred-constraint/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/graph-inferred-constraint/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/graph-inferred-constraint/exact.json"),
        ),
    ]);
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for depth in [0, 1] {
            let answer = project.type_of(TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(
                        SOURCE,
                        SOURCE.find("/*#query*/").unwrap() + "/*#query*/".len(),
                    ),
                },
                expand: true,
                depth,
            });
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{config}/{depth}: {answer:?}");
            };
            assert_eq!(result.description.summary, "U");
            if depth == 0 {
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert!(result.issues.is_empty());
            } else {
                assert_eq!(result.outcome, TypeOutcome::Partial);
                assert_eq!(result.issues.len(), 1, "{config}: {:?}", result.issues);
                assert_eq!(result.issues[0].reason, SemanticFailureReason::FlowRequired);
                assert_eq!(
                    result.issues[0].location.start,
                    position(
                        SOURCE,
                        SOURCE.find("return external").unwrap() + "return ".len()
                    )
                );
                let graph = result.description.expansion.unwrap();
                assert_eq!(
                    graph
                        .nodes
                        .iter()
                        .filter(|node| matches!(node.data, TypeNodeData::Unsupported { .. }))
                        .count(),
                    1
                );
            }
        }
    }
}

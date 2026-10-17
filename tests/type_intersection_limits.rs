//! Native cross-product complexity differs from a long flat shallow result.
mod support;

use support::Fixture;
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

fn union(key: &str, count: usize) -> String {
    (0..count)
        .map(|index| format!("{{ {key}: {index} }}"))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn fixture(source: &str) -> Fixture {
    Fixture::new(&[
        ("cases.ts", source),
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true},"files":["cases.ts"]}"#,
        ),
        (
            "loose.json",
            r#"{"extends":"./tsconfig.json","compilerOptions":{"strict":false}}"#,
        ),
    ])
}

fn request(fixture: &Fixture) -> TypeRequest {
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: SourcePosition {
            line: 1,
            column: 15,
        },
    });
    request.expand = true;
    request.depth = 0;
    request
}

#[test]
fn natural_intersection_cross_product_limit_is_invalid_at_and_above_100000() {
    for (left, right, count) in [(369, 271, 99999), (400, 250, 100000), (11, 9091, 100001)] {
        let source = format!(
            "declare const root: ({}) & ({});\n",
            union("left", left),
            union("right", right)
        );
        let fixture = fixture(&source);
        for config in ["tsconfig.json", "loose.json"] {
            let project = fixture.load_config(config);
            let answer = project.type_of(request(&fixture));
            if count < 100000 {
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!(
                        "native just-under cross product must establish its real union ({config}): {answer:?}"
                    );
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert!(result.issues.is_empty());
                let graph = result.description.expansion.unwrap();
                let root = graph
                    .nodes
                    .iter()
                    .find(|node| node.id == graph.root)
                    .unwrap();
                let TypeNodeData::Union {
                    alternatives,
                    omitted_alternatives,
                } = &root.data
                else {
                    panic!("actual under-limit cross product is a union");
                };
                assert!(alternatives.is_empty());
                assert_eq!(*omitted_alternatives, Some(count));
                assert_eq!(graph.nodes.len(), 1);
                assert!(graph.truncated);
            } else {
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error))
                    if error.kind == SemanticErrorKind::InvalidSemantic
                    && error.reason == SemanticFailureReason::UnionComplexityLimit.into()),
                    "native cross-product errorType recovery must stay a distinct invalid cause ({config}/{count}): {answer:?}"
                );
            }
        }
    }
}

#[test]
fn a_long_flat_intersection_can_normalize_to_one_shallow_native_type() {
    let source = format!(
        "declare const root: {};\n",
        vec!["number"; 100001].join(" & ")
    );
    let fixture = fixture(&source);
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        let answer = project.type_of(request(&fixture));
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!(
                "flat normalization does not consume the cartesian limit ({config}): {answer:?}"
            );
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(result.description.summary, "number");
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        assert_eq!(graph.nodes.len(), 1);
        assert!(!graph.truncated);
    }
}

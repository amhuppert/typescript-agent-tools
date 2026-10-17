//! Native template complexity limits differ from input span count.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest,
};

// Exact equivalent of compiler-limits/generate.mjs crossProductSource.
fn product(left: usize, right: usize) -> String {
    let union = |prefix, length| {
        (0..length)
            .map(|index| format!("'{prefix}{index}'"))
            .collect::<Vec<_>>()
            .join(" | ")
    };
    format!(
        "export {{}};\ntype Left = {};\ntype Right = {};\ntype /*#query*/Result = `${{Left}}${{Right}}`;\n",
        union("L", left),
        union("R", right)
    )
}

// Exact equivalent of compiler-limits/generate.mjs crossProductOverflowSource.
fn overflowing_product(factors: usize, never_position: &str) -> String {
    let mut spans = "${Choice}".repeat(factors);
    match never_position {
        "first" => spans.insert_str(0, "${never}"),
        "last" => spans.push_str("${never}"),
        "none" => {}
        _ => panic!("invalid Never position"),
    }
    format!("export {{}};\ntype Choice = 'a' | 'b';\ntype /*#query*/Result = `{spans}`;\n")
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
fn request(fixture: &Fixture, source: &str) -> TypeRequest {
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(source, source.find("/*#query*/").unwrap() + 10),
        },
        expand: true,
        depth: 0,
    }
}
#[test]
fn template_cross_product_preserves_native_boundary_and_actual_union_size() {
    for (left, right, count) in [(3, 33333, 99999), (100, 1000, 100000), (11, 9091, 100001)] {
        let source = product(left, right);
        let fixture = fixture(&source);
        for config in ["tsconfig.json", "loose.json"] {
            let answer = fixture
                .load_config(config)
                .type_of(request(&fixture, &source));
            if count >= 100000 {
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnionComplexityLimit.into()),
                    "{config}/{count}: {answer:?}"
                );
            } else {
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{count}: {answer:?}");
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
                    panic!("{root:?}");
                };
                assert!(alternatives.is_empty());
                assert_eq!(*omitted_alternatives, Some(count));
                assert_eq!(graph.nodes.len(), 1);
                assert!(graph.truncated);
            }
        }
    }
}
#[test]
fn ten_thousand_flat_placeholders_keep_a_shallow_literal_result() {
    let source = format!("type /*#query*/Result = `{}`;\n", "${\"\"}".repeat(10001));
    let fixture = fixture(&source);
    for config in ["tsconfig.json", "loose.json"] {
        let answer = fixture
            .load_config(config)
            .type_of(request(&fixture, &source));
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!("{config}: {answer:?}");
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(result.description.summary, "\"\"");
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(
            graph.nodes[0].data,
            TypeNodeData::Literal {
                base: PrimitiveName::String,
                value: String::new().into()
            }
        );
        assert!(!graph.truncated);
    }
}

#[test]
fn signed_product_overflow_cannot_be_erased_by_a_later_never() {
    // A large nonoverflowing product followed by Never is deliberately absent:
    // native maps that empty product exponentially. The small late-Never case
    // and the leading-Never cases prove the healthy zero-product branches.
    for (factors, never_position, failed) in [
        (62, "none", true),
        (63, "none", true),
        (64, "none", true),
        (63, "last", true),
        (64, "last", true),
        (62, "first", false),
        (63, "first", false),
        (64, "first", false),
        (3, "last", false),
    ] {
        let source = overflowing_product(factors, never_position);
        let fixture = fixture(&source);
        for config in ["tsconfig.json", "loose.json"] {
            let answer = fixture
                .load_config(config)
                .type_of(request(&fixture, &source));
            if failed {
                assert!(
                    matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnionComplexityLimit.into()),
                    "{config}/{factors}/{never_position}: {answer:?}"
                );
            } else {
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{factors}/{never_position}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(result.description.summary, "never");
                assert!(result.issues.is_empty());
                let graph = result.description.expansion.unwrap();
                assert_eq!(graph.nodes.len(), 1);
                assert!(!graph.truncated);
            }
        }
    }
}

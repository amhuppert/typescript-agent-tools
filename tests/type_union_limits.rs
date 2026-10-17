//! Native subtype work estimate: integer quotient before multiplication, > not >=.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

// Exact equivalent of fixtures/types/compiler-limits/generate.mjs subtypeRemovalSource.
fn source(length: usize, removals: usize) -> String {
    let mut rows = Vec::new();
    let mut names = Vec::new();
    for i in 0..length - removals {
        rows.push(format!("declare const base{i}: {{tag: {i}}};"));
        names.push(format!("base{i}"));
    }
    for i in 0..removals {
        rows.push(format!(
            "declare const redundant{i}: {{tag: 0; extra{i}: 0}};"
        ));
        names.push(format!("redundant{i}"));
    }
    format!(
        "{}\nconst /*#query*/query = [{}];\n",
        rows.join("\n"),
        names.join(",")
    )
}
fn fixture(source: &str) -> Fixture {
    Fixture::new(&[
        ("cases.ts", source),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-array-expressions/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-array-expressions/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-array-expressions/loose.json"),
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
        depth: 1,
    }
}
#[test]
fn subtype_estimate_accepts_at_threshold_and_rejects_just_above() {
    for (removals, constituents, invalid) in [(43, 1207, false), (42, 1208, false), (41, 1, true)] {
        let source = source(1250, removals);
        let fixture = fixture(&source);
        for config in ["tsconfig.json", "loose.json"] {
            let answer = fixture
                .load_config(config)
                .type_of(request(&fixture, &source));
            if invalid {
                assert!(
                    matches!(&answer,Err(AnalysisError::Semantic(error)) if error.kind==SemanticErrorKind::InvalidSemantic && error.reason==SemanticFailureReason::SubtypeComplexityLimit.into()),
                    "{config}/{removals}: {answer:?}"
                );
            } else {
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{removals}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert!(result.issues.is_empty());
                let graph = result.description.expansion.unwrap();
                assert!(graph.nodes.iter().any(|node|matches!(&node.data,TypeNodeData::Union {alternatives,omitted_alternatives} if alternatives.is_empty() && *omitted_alternatives==Some(constituents))),"actual under-limit union count {constituents}: {graph:?}");
            }
        }
    }
}
#[test]
fn a_long_flat_array_of_one_type_does_not_consume_subtype_pairs() {
    let source = format!(
        "declare const value: number;\nconst /*#query*/query=[{}];\n",
        vec!["value"; 10001].join(",")
    );
    let fixture = fixture(&source);
    for config in ["tsconfig.json", "loose.json"] {
        let answer = fixture
            .load_config(config)
            .type_of(request(&fixture, &source));
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!("{config}: {answer:?}");
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(result.description.summary, "number[]");
        assert!(result.issues.is_empty());
    }
}

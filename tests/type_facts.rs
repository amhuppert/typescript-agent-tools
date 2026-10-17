//! Native type facts are consumed through final unary Project outcomes.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/facts-core/cases.ts");
const STRICT: &str = include_str!("../fixtures/types/facts-core/tsconfig.json");
const LOOSE: &str = include_str!("../fixtures/types/facts-core/loose.json");

fn check(cases: &[(&str, &str, &str)]) {
    let fixture = Fixture::new(&[
        ("cases.ts", SOURCE),
        ("tsconfig.json", STRICT),
        ("loose.json", LOOSE),
    ]);
    for (strict, config) in [(false, "loose.json"), (true, "tsconfig.json")] {
        let project = fixture.load_config(config);
        for &(name, strict_summary, loose_summary) in cases {
            let mut request = TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(SOURCE, SOURCE.find(name).unwrap()),
            });
            request.expand = true;
            let answer = project.type_of(request);
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "native union facts must be available to unary checking for {name}, strict={strict}: {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            let expected = if strict {
                strict_summary
            } else {
                loose_summary
            };
            assert_eq!(result.description.summary, expected);
            let graph = result.description.expansion.unwrap();
            assert_eq!(graph.nodes.len(), 1);
            assert!(!graph.truncated);
            assert_eq!(
                graph.nodes[0].data,
                if expected == "boolean" {
                    TypeNodeData::Primitive {
                        name: PrimitiveName::Boolean,
                    }
                } else {
                    TypeNodeData::Literal {
                        base: PrimitiveName::Boolean,
                        value: expected.into(),
                    }
                }
            );
        }
    }
}

#[test]
fn primitive_and_literal_unions_use_the_compilers_whole_fact_mask() {
    check(&[
        ("negatedLiteral", "boolean", "boolean"),
        ("negatedTruthy", "false", "boolean"),
        ("negatedPrimitive", "boolean", "boolean"),
        ("negatedMixed", "boolean", "boolean"),
    ]);
}

#[test]
fn nullable_and_never_fact_masks_keep_their_distinct_unary_results() {
    check(&[
        ("negatedNullable", "true", "true"),
        ("negatedNever", "boolean", "boolean"),
    ]);
}

//! Final outcomes for the pinned null/void initializer and unary questions.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeQuestion, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/special-expressions/cases.ts");
const CONFIG: &str = include_str!("../fixtures/types/special-expressions/tsconfig.json");

fn check_cases(cases: &[(&str, &str)]) {
    for (strict, implicit) in [(true, true), (true, false), (false, true), (false, false)] {
        let config = CONFIG.replace(
            "\"strict\": true",
            &format!("\"strict\": {strict}, \"noImplicitAny\": {implicit}"),
        );
        let fixture = Fixture::new(&[("tsconfig.json", &config), ("cases.ts", SOURCE)]);
        let project = fixture.load();
        for &(name, strict_summary) in cases {
            let mut request = TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(SOURCE, SOURCE.find(name).unwrap()),
            });
            request.expand = true;
            let answer = project.type_of(request);
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "{name} under strict={strict} must establish the pinned native answer: {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            let strict_summary = if strict
                && !implicit
                && matches!(
                    name,
                    "localMutableNull" | "localParenthesizedNull" | "localVarNull"
                ) {
                "null"
            } else {
                strict_summary
            };
            let summary = if !strict && matches!(strict_summary, "null" | "undefined") {
                "any"
            } else {
                strict_summary
            };
            assert_eq!(result.target.kind, TypeQuestion::Value);
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(
                result.description.summary, summary,
                "{name} strict={strict}"
            );
            assert!(result.issues.is_empty());
            let graph = result.description.expansion.unwrap();
            assert_eq!(graph.nodes.len(), 1);
            assert!(!graph.truncated);
            let expected = match summary {
                "null" => TypeNodeData::Null,
                "undefined" => TypeNodeData::Undefined,
                "any" => TypeNodeData::Any,
                "number" => TypeNodeData::Primitive {
                    name: PrimitiveName::Number,
                },
                "true" => TypeNodeData::Literal {
                    base: PrimitiveName::Boolean,
                    value: "true".into(),
                },
                _ => panic!("unrecognized pinned summary"),
            };
            assert_eq!(graph.nodes[0].data, expected);
        }
    }
}

#[test]
fn null_initializer_widening_and_local_automatic_type_follow_the_compiler_options() {
    check_cases(&[
        ("constantNull", "null"),
        ("mutableNull", "null"),
        ("localMutableNull", "any"),
        ("localConstantNull", "null"),
        ("localParenthesizedNull", "any"),
        ("localVarNull", "any"),
        ("exportedVarNull", "null"),
        ("localUninitialized", "any"),
        ("exportedUninitialized", "any"),
    ]);
}

#[test]
fn void_initializers_do_not_demand_the_scheduled_operand() {
    check_cases(&[
        ("constantVoid", "undefined"),
        ("mutableVoid", "undefined"),
        ("deferredVoid", "undefined"),
        ("localMutableVoid", "undefined"),
    ]);
}

#[test]
fn truthiness_and_bigint_unary_results_preserve_unconsumed_diagnostics() {
    check_cases(&[
        ("negatedNull", "true"),
        ("negatedVoid", "true"),
        ("positiveBigint", "number"),
    ]);
}

#[test]
fn the_null_expression_token_retains_its_type_before_variable_widening() {
    for strict in [true, false] {
        let config = CONFIG.replace("\"strict\": true", &format!("\"strict\": {strict}"));
        let fixture = Fixture::new(&[("tsconfig.json", &config), ("cases.ts", SOURCE)]);
        let project = fixture.load();
        let offset = SOURCE.find("= null").unwrap() + 2;
        let mut request = TypeRequest::new(TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, offset),
        });
        request.expand = true;
        let answer = project.type_of(request);
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "the actual null expression token must be selected: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.target.kind, TypeQuestion::Expression);
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(result.description.summary, "null");
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        assert_eq!(graph.nodes[0].data, TypeNodeData::Null);
    }
}

#[test]
fn truthiness_uses_complete_anonymous_shape_and_actual_interface_kind() {
    for strict in [true, false] {
        let config = CONFIG.replace("\"strict\": true", &format!("\"strict\": {strict}"));
        let fixture = Fixture::new(&[("tsconfig.json", &config), ("cases.ts", SOURCE)]);
        let project = fixture.load();
        for name in [
            "negatedCallable",
            "negatedConstructor",
            "negatedIndexed",
            "negatedAnonymous",
            "negatedNominal",
        ] {
            let request = TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(SOURCE, SOURCE.find(name).unwrap()),
            });
            let answer = project.type_of(request);
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "native structural truthiness must be established ({strict}, {name}): {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(
                result.description.summary,
                if strict && name != "negatedAnonymous" {
                    "false"
                } else {
                    "boolean"
                },
                "strict={strict}, {name}"
            );
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
        }
    }
}

#[test]
fn numeric_unary_results_preserve_consumed_nullable_recovery() {
    for strict in [true, false] {
        for implicit in [true, false] {
            let config = CONFIG.replace(
                "\"strict\": true",
                &format!("\"strict\": {strict}, \"noImplicitAny\": {implicit}"),
            );
            let fixture = Fixture::new(&[("tsconfig.json", &config), ("cases.ts", SOURCE)]);
            let project = fixture.load();
            for name in [
                "positiveNull",
                "negativeNull",
                "invertedNull",
                "positiveVoid",
                "negativeVoid",
                "invertedVoid",
            ] {
                let start = SOURCE.find(name).unwrap();
                let operand = start
                    + SOURCE[start..].find(" = ").unwrap()
                    + 4
                    + usize::from(name.ends_with("Void"));
                for expand in [false, true] {
                    let mut request = TypeRequest::new(TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, start),
                    });
                    request.expand = expand;
                    let answer = project.type_of(request);
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{strict}/{implicit}/{name}/{expand}: {answer:?}")
                    };
                    assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
                    assert_eq!(error.reason, SemanticFailureReason::ErrorType.into());
                    assert_eq!(
                        error.position,
                        Some(position(SOURCE, operand)),
                        "{name}: {error:?}"
                    );
                    assert!(error.causes.is_empty(), "{name}: {error:?}");
                }
            }
        }
    }
}

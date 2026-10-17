//! Final Project answers distinguish syntax-directed unary literal results.
mod support;

use support::Fixture;
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest,
};

const VALUES: &str = include_str!("../fixtures/types/constants/cases.ts");
const IMPORTS: &str = include_str!("../fixtures/types/constants/consumer.ts");
const CONFIG: &str = include_str!("../fixtures/types/constants/tsconfig.json");
const EXPECTATIONS: &str = include_str!("../fixtures/types/constants/expectations.json");

fn base(text: &str) -> PrimitiveName {
    match text {
        "string" => PrimitiveName::String,
        "number" => PrimitiveName::Number,
        "boolean" => PrimitiveName::Boolean,
        "bigint" => PrimitiveName::Bigint,
        _ => panic!("unknown fixture primitive"),
    }
}

fn check_family(feature: &str) {
    check_profile(feature, true);
}

fn check_profile(feature: &str, strict: bool) {
    let nonstrict = CONFIG.replace("\"strict\": true", "\"strict\": false");
    let fixture = Fixture::new(&[
        ("tsconfig.json", if strict { CONFIG } else { &nonstrict }),
        ("cases.ts", VALUES),
        ("consumer.ts", IMPORTS),
    ]);
    let loaded = fixture.load();
    let rows: Vec<serde_json::Value> = serde_json::from_str(EXPECTATIONS).unwrap();
    for row in rows.iter().filter(|row| row["feature"] == feature) {
        let name = row["name"].as_str().unwrap();
        let offset = IMPORTS
            .find(&format!("{name},"))
            .unwrap_or_else(|| IMPORTS.find(&format!("{name} }}")).unwrap());
        let answer = loaded.type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("consumer.ts"),
                position: SourcePosition {
                    line: 1,
                    column: offset + 1,
                },
            },
            expand: true,
            depth: 0,
        });
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "{name} must establish the compiler's native constant type: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        let summary = if strict {
            &row["summary"]
        } else {
            &row["nonStrictSummary"]
        };
        assert_eq!(
            result.description.summary,
            summary.as_str().unwrap(),
            "{name}"
        );
        assert!(!result.description.summary_truncated);
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        assert!(!graph.truncated);
        assert_eq!(graph.nodes.len(), 1);
        let primitive = base(row["base"].as_str().unwrap());
        let kind = if strict {
            &row["kind"]
        } else {
            &row["nonStrictKind"]
        };
        let expected = if kind == "literal" {
            TypeNodeData::Literal {
                base: primitive,
                value: row["value"].as_str().unwrap().into(),
            }
        } else {
            TypeNodeData::Primitive { name: primitive }
        };
        assert_eq!(graph.nodes[0].data, expected, "{name}");
    }
}

#[test]
fn boolean_literals_and_mutable_widening_keep_the_compiler_type() {
    check_family("boolean");
}

#[test]
fn no_substitution_templates_use_the_parser_cooked_payload() {
    check_family("template");
}

#[test]
fn bigint_radices_magnitudes_and_negative_zero_have_exact_normalized_values() {
    check_family("bigint");
}

#[test]
fn parentheses_direct_numeric_unary_and_mutable_coercion_have_distinct_results() {
    check_family("unary");
}

#[test]
fn negation_uses_type_facts_including_the_empty_object_case() {
    check_family("truthiness");
}

#[test]
fn non_strict_negation_keeps_nullable_facts_for_nonempty_and_nonzero_literals() {
    check_profile("truthiness", false);
}

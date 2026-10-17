//! Final native literal answers use pinned compiler values and printed text.
mod support;

use std::path::Path;
use support::project;
use typescript_parser::analysis::AnalysisError;
use typescript_parser::project::Project;
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeLiteralPart, TypeLiteralValue, TypeNodeData, TypeOutcome,
    TypeQuery, TypeRequest, TypeResult,
};

const VALUES: &str = include_str!("../fixtures/types/literals/cases.ts");
const IMPORTS: &str = include_str!("../fixtures/types/literals/consumer.ts");
const EXPECTATIONS: &str = include_str!("../fixtures/types/literals/expectations.json");

fn query(project: &Project, file: &Path, name: &str) -> Result<TypeAnswer, AnalysisError> {
    let offset = IMPORTS
        .find(&format!("{name},"))
        .unwrap_or_else(|| IMPORTS.find(&format!("{name} }}")).unwrap());
    project.type_of(TypeRequest {
        query: TypeQuery::At {
            file: file.into(),
            position: SourcePosition {
                line: 1,
                column: offset + 1,
            },
        },
        expand: true,
        depth: 0,
    })
}

fn selected(project: &Project, file: &Path, name: &str) -> TypeResult {
    let answer = query(project, file, name);
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(_))),
        "{name} must retain its exact native literal: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    result
}

fn assert_literal(project: &Project, file: &Path, row: &serde_json::Value) {
    let name = row["name"].as_str().unwrap();
    let result = selected(project, file, name);
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert_eq!(
        result.target.kind,
        typescript_parser::types::TypeQuestion::Value
    );
    assert_eq!(result.target.location.file, "consumer.ts");
    assert_eq!(
        result.description.summary,
        row["summary"].as_str().unwrap(),
        "{name}"
    );
    assert!(!result.description.summary_truncated);
    assert!(result.issues.is_empty());
    let graph = result.description.expansion.unwrap();
    assert_eq!(graph.nodes.len(), 1);
    assert!(!graph.truncated);
    assert_eq!(graph.nodes[0].omitted, None);
    assert_eq!(
        graph.nodes[0].data,
        TypeNodeData::Literal {
            base: if row["base"] == "string" {
                PrimitiveName::String
            } else {
                PrimitiveName::Number
            },
            value: row["value"].as_str().unwrap().into()
        },
        "{name}"
    );
}

#[test]
fn string_literal_cooking_preserves_every_escape_and_line_continuation_variant() {
    let fixture = project(&[("cases.ts", VALUES), ("consumer.ts", IMPORTS)]);
    let loaded = fixture.load();
    let expected: Vec<serde_json::Value> = serde_json::from_str(EXPECTATIONS).unwrap();
    let strings: Vec<_> = expected
        .iter()
        .filter(|row| row["base"] == "string")
        .collect();
    assert_eq!(strings.len(), 17);
    for row in strings {
        assert_literal(&loaded, &fixture.path("consumer.ts"), row);
    }
}

#[test]
fn literal_summaries_use_the_shared_typescript_escape_owner() {
    let fixture = project(&[("cases.ts", VALUES), ("consumer.ts", IMPORTS)]);
    let loaded = fixture.load();
    // Already admitted raw U+0085 distinguishes TypeScript EscapeString from
    // generic JSON escaping before any new cooking branch is implemented.
    let result = selected(
        &loaded,
        &fixture.path("consumer.ts"),
        "string_raw_next_line",
    );
    assert_eq!(result.description.summary, "\"\\u0085\"");
    assert_eq!(
        result.description.expansion.unwrap().nodes[0].data,
        TypeNodeData::Literal {
            base: PrimitiveName::String,
            value: "\u{85}".into()
        }
    );
}

#[test]
fn numeric_literals_use_the_scanner_normalization_for_all_radices_and_rounding() {
    let fixture = project(&[("cases.ts", VALUES), ("consumer.ts", IMPORTS)]);
    let loaded = fixture.load();
    let expected: Vec<serde_json::Value> = serde_json::from_str(EXPECTATIONS).unwrap();
    let numbers: Vec<_> = expected
        .iter()
        .filter(|row| row["base"] == "number")
        .collect();
    assert_eq!(numbers.len(), 22);
    for row in numbers {
        assert_literal(&loaded, &fixture.path("consumer.ts"), row);
    }
}

#[test]
fn unpaired_literal_units_preserve_exact_native_bytes_without_identity_merging() {
    let fixture = project(&[("cases.ts", VALUES), ("consumer.ts", IMPORTS)]);
    let loaded = fixture.load();
    // The pinned compiler accepts these values. The native-byte projection
    // now represents both exactly; the old scalar-only rejection is obsolete.
    for (name, summary, bytes) in [
        ("lone_high", "\"\\uD800\"", "eda080"),
        ("lone_low", "\"\\uDC00\"", "edb080"),
    ] {
        let result = selected(&loaded, &fixture.path("consumer.ts"), name);
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(result.description.summary, summary);
        assert!(!result.description.summary_truncated);
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        assert!(!graph.truncated);
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(
            graph.nodes[0].data,
            TypeNodeData::Literal {
                base: PrimitiveName::String,
                value: TypeLiteralValue::Symbolic {
                    parts: vec![TypeLiteralPart::Bytes {
                        value: bytes.to_owned(),
                    }],
                },
            },
            "{name}",
        );
    }
}

//! Final answers preserve the compiler's template normalization and demand order.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-template-types/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-template-types/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-template-types/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-template-types/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-template-types/exact.json"),
        ),
    ])
}
fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth: 0,
    }
}
#[test]
fn template_normalization_substitution_and_unions_keep_native_answers() {
    let fixture = fixture();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/types/native-template-types/expected.json"
    ))
    .unwrap();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for row in expected[config].as_array().unwrap() {
            let marker = row["marker"].as_str().unwrap();
            if ["missing", "checkedError", "missingFirst", "neverMissing"].contains(&marker) {
                continue;
            }
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{config}/{marker}/{expand}: {:?}",
                    result.issues
                );
                assert_eq!(
                    result.description.summary,
                    row["summary"].as_str().unwrap(),
                    "{config}/{marker}/{expand}"
                );
                assert!(result.issues.is_empty());
                if let Some(graph) = result.description.expansion {
                    let root = &graph.nodes[0];
                    if let Some(alias) = &root.alias {
                        let origin = &alias.origins[0];
                        assert_eq!(
                            root.summary,
                            format!(
                                "{} ({}:{}:{})",
                                result.description.summary,
                                origin.file,
                                origin.start.line,
                                origin.start.column
                            )
                        );
                    } else {
                        assert_eq!(root.summary, result.description.summary);
                    }
                }
            }
        }
    }
}
#[test]
fn surviving_surrogate_sentinels_keep_exact_literal_bytes() {
    use typescript_parser::types::{TypeLiteralPart, TypeLiteralValue, TypeNodeData};
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        // Native EncodeJSStringRune retains each unpaired half. Only an
        // adjacent high/low pair is combined by getTemplateLiteralType.
        for (marker, bytes) in [("reversed", "edb880eda0bd"), ("doubleHigh", "eda0bdeda0bd")] {
            let answer = project.type_of(request(&fixture, marker, true)).unwrap();
            let TypeAnswer::Selected(result) = answer else {
                panic!("{answer:?}");
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            let graph = result.description.expansion.unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            let TypeNodeData::Literal { value, .. } = &root.data else {
                panic!("{root:?}");
            };
            assert_eq!(
                value,
                &TypeLiteralValue::Symbolic {
                    parts: vec![TypeLiteralPart::Bytes {
                        value: bytes.into()
                    }],
                },
                "{config}/{marker}"
            );
        }
    }
}
#[test]
fn acquisition_errors_precede_text_normalization_and_survive_source_checks() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for marker in ["missing", "checkedError", "missingFirst", "neverMissing"] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
                assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
                assert_eq!(error.path, Some(fixture.path("cases.ts")));
                let offset = SOURCE.find(&format!("/*#{marker}*/")).unwrap();
                let body = offset + SOURCE[offset..].find('=').unwrap() + 1;
                assert_eq!(
                    error.position,
                    Some(position(
                        SOURCE,
                        body + SOURCE[body..].find("Missing").unwrap()
                    ))
                );
                let healthy = project.type_of(request(&fixture, "joined", expand));
                let Ok(TypeAnswer::Selected(result)) = healthy else {
                    panic!("healthy query after {marker}: {healthy:?}");
                };
                assert_eq!(result.description.summary, "\"😀\"");
            }
        }
    }
}
#[test]
fn template_expansion_exposes_placeholder_type_edges() {
    use typescript_parser::types::{PrimitiveName, TypeNodeData, TypeOperator, TypeRef};
    let fixture = fixture();
    let project = fixture.load_config("tsconfig.json");
    let mut query = request(&fixture, "pattern", true);
    query.depth = 1;
    let answer = project.type_of(query).unwrap();
    let TypeAnswer::Selected(result) = answer else {
        panic!("{answer:?}");
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert!(result.issues.is_empty());
    let graph = result.description.expansion.unwrap();
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    let TypeNodeData::Operator {
        operator,
        operands,
        parameters,
        ..
    } = &root.data
    else {
        panic!("{root:?}");
    };
    assert_eq!(*operator, TypeOperator::TemplateLiteral);
    assert!(parameters.is_empty());
    assert_eq!(operands.len(), 1);
    let TypeRef::Ref { r#ref } = &operands[0] else {
        panic!("{operands:?}");
    };
    let child = graph.nodes.iter().find(|node| &node.id == r#ref).unwrap();
    assert_eq!(
        child.data,
        TypeNodeData::Primitive {
            name: PrimitiveName::Number
        }
    );
    assert!(!graph.truncated);
}

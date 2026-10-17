//! Intrinsic identity and pinned Unicode casing survive the final public query.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-string-mappings/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-string-mappings/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-string-mappings/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-string-mappings/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-string-mappings/exact.json"),
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
fn native_intrinsics_casing_templates_and_reduction_keep_exact_answers() {
    let fixture = fixture();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/types/native-string-mappings/expected.json"
    ))
    .unwrap();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for row in expected[config].as_array().unwrap() {
            let marker = row["marker"].as_str().unwrap();
            if marker == "missing" {
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
fn mapping_arguments_keep_error_dependencies_and_query_isolation() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, "missing", expand));
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{config}/{expand}: {answer:?}");
            };
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
            assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
            assert_eq!(error.path, Some(fixture.path("cases.ts")));
            assert_eq!(
                error.position,
                Some(position(
                    SOURCE,
                    SOURCE.find("Uppercase<Missing>").unwrap() + "Uppercase<".len()
                ))
            );
            let healthy = project.type_of(request(&fixture, "pairMapping", expand));
            let Ok(TypeAnswer::Selected(result)) = healthy else {
                panic!("healthy query after missing: {healthy:?}");
            };
            assert_eq!(result.description.summary, "\"😀\"");
        }
    }
}
#[test]
fn nested_mapping_expansion_preserves_both_operator_targets() {
    use typescript_parser::types::{PrimitiveName, TypeNodeData, TypeOperator, TypeRef};
    let fixture = fixture();
    let project = fixture.load_config("tsconfig.json");
    let mut query = request(&fixture, "nested", true);
    query.depth = 2;
    let answer = project.type_of(query).unwrap();
    let TypeAnswer::Selected(result) = answer else {
        panic!("{answer:?}");
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert!(result.issues.is_empty());
    let graph = result.description.expansion.unwrap();
    let mut id = &graph.root;
    for summary in ["Uppercase<Lowercase<string>>", "Lowercase<string>"] {
        let node = graph.nodes.iter().find(|node| &node.id == id).unwrap();
        assert_eq!(node.summary, summary);
        let TypeNodeData::Operator {
            operator,
            operands,
            parameters,
            ..
        } = &node.data
        else {
            panic!("{node:?}");
        };
        assert_eq!(*operator, TypeOperator::StringMapping);
        assert!(parameters.is_empty());
        assert_eq!(operands.len(), 1);
        let TypeRef::Ref { r#ref } = &operands[0] else {
            panic!("{operands:?}");
        };
        id = r#ref;
    }
    let leaf = graph.nodes.iter().find(|node| &node.id == id).unwrap();
    assert_eq!(
        leaf.data,
        TypeNodeData::Primitive {
            name: PrimitiveName::String
        }
    );
    assert!(!graph.truncated);
}

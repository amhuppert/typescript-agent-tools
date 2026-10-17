//! Shared intersection reduction preserves symbolic identities and demand failures.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-template-intersections/cases.ts");
fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "globals.d.ts",
            include_str!("../fixtures/types/native-template-intersections/globals.d.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-template-intersections/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-template-intersections/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-template-intersections/exact.json"),
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
fn assert_native_summaries(include: impl Fn(&str) -> bool) {
    let fixture = fixture();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/types/native-template-intersections/expected.json"
    ))
    .unwrap();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for row in expected[config].as_array().unwrap() {
            let marker = row["marker"].as_str().unwrap();
            if !include(marker) {
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
            }
        }
    }
}

#[test]
fn string_intersections_reduce_in_native_order_with_generic_distinctions() {
    assert_native_summaries(|marker| {
        !matches!(marker, "missing" | "versionLower" | "versionUpper")
    });
}

#[test]
fn casing_uses_the_compilers_unicode_version() {
    assert_native_summaries(|marker| matches!(marker, "versionLower" | "versionUpper"));
}

#[test]
fn unresolved_pattern_dependency_is_not_hidden_by_literal_reduction() {
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
            assert_eq!(
                error.position,
                Some(position(
                    SOURCE,
                    SOURCE.find("MissingIntersection").unwrap()
                ))
            );
            let healthy = project.type_of(request(&fixture, "match", expand));
            let Ok(TypeAnswer::Selected(result)) = healthy else {
                panic!("{config}: {healthy:?}");
            };
            assert_eq!(result.description.summary, "\"getX\"");
        }
    }
}
#[test]
fn same_named_mapping_symbols_keep_distinct_graph_nodes() {
    use typescript_parser::types::{TypeNodeData, TypeOperator, TypeRef};
    let fixture = fixture();
    let project = fixture.load_config("tsconfig.json");
    let mut query = request(&fixture, "distinctMappings", true);
    query.depth = 2;
    let answer = project.type_of(query).unwrap();
    let TypeAnswer::Selected(result) = answer else {
        panic!("{answer:?}");
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    let graph = result.description.expansion.unwrap();
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    let TypeNodeData::Union { alternatives, .. } = &root.data else {
        panic!("{root:?}");
    };
    assert_eq!(alternatives.len(), 2);
    assert_ne!(alternatives[0], alternatives[1]);
    for alternative in alternatives {
        let TypeRef::Ref { r#ref } = alternative else {
            panic!("{alternative:?}");
        };
        let node = graph.nodes.iter().find(|node| &node.id == r#ref).unwrap();
        assert!(matches!(
            &node.data,
            TypeNodeData::Operator {
                operator: TypeOperator::StringMapping,
                ..
            }
        ));
    }
}

#[test]
fn distributed_intersection_retains_only_matching_literal_members() {
    use typescript_parser::types::{PrimitiveName, TypeNodeData, TypeRef};
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        let mut query = request(&fixture, "union", true);
        query.depth = 1;
        let TypeAnswer::Selected(result) = project.type_of(query).unwrap() else {
            panic!("unselected union");
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
        assert_eq!(*omitted_alternatives, None);
        let values = alternatives
            .iter()
            .map(|reference| {
                let TypeRef::Ref { r#ref } = reference else {
                    panic!("{reference:?}");
                };
                let node = graph.nodes.iter().find(|node| &node.id == r#ref).unwrap();
                let TypeNodeData::Literal {
                    base: PrimitiveName::String,
                    value,
                } = &node.data
                else {
                    panic!("{node:?}");
                };
                value.as_text().expect("ordinary literal text")
            })
            .collect::<Vec<_>>();
        assert_eq!(values, ["a1", "b2"], "{config}");
        assert!(!graph.truncated);
    }
}

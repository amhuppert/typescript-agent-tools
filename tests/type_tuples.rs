//! Native tuple targets and normalization observed through final Project queries.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/native-algebra/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-algebra/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-algebra/loose.json"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str, expand: bool, depth: usize) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
    });
    request.expand = expand;
    request.depth = depth;
    request
}

#[test]
fn tuple_summaries_preserve_optional_rest_variadic_labels_and_readonly() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        let strict = config == "tsconfig.json";
        for (marker, expected) in [
            ("tupleEmpty", "[]"),
            ("tupleFixed", "[string, number]"),
            (
                "tupleOptional",
                if strict {
                    "[string, (number | undefined)?]"
                } else {
                    "[string, number?]"
                },
            ),
            (
                "tupleLabelled",
                if strict {
                    "[title: string, count?: number | undefined]"
                } else {
                    "[title: string, count?: number]"
                },
            ),
            (
                "tupleReadonly",
                if strict {
                    "readonly [title: string, count?: number | undefined]"
                } else {
                    "readonly [title: string, count?: number]"
                },
            ),
            ("tupleFixedRest", "[string, ...number[]]"),
            ("tupleVariadic", "[string, ...V]"),
            ("tupleEscapedLabel", "[title: string]"),
        ] {
            for (expand, depth) in [(false, 0), (true, 0), (true, 2)] {
                let answer = project.type_of(request(&fixture, marker, expand, depth));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{depth}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{marker}");
                assert_eq!(result.description.summary, expected, "{config}/{marker}");
                assert!(result.issues.is_empty(), "{config}/{marker}");
                if let Some(graph) = result.description.expansion {
                    let root = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == graph.root)
                        .unwrap();
                    assert!(matches!(root.data, TypeNodeData::Tuple { .. }), "{root:?}");
                }
            }
        }
    }
}

#[test]
fn tuple_graph_elements_retain_native_flags_and_actual_argument_edges() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, readonly, flags, labels) in [
            ("tupleEmpty", false, vec![], vec![]),
            (
                "tupleFixed",
                false,
                vec![(false, false, false), (false, false, false)],
                vec![None, None],
            ),
            (
                "tupleOptional",
                false,
                vec![(false, false, false), (true, false, false)],
                vec![None, None],
            ),
            (
                "tupleLabelled",
                false,
                vec![(false, false, false), (true, false, false)],
                vec![Some("title"), Some("count")],
            ),
            (
                "tupleReadonly",
                true,
                vec![(false, false, false), (true, false, false)],
                vec![Some("title"), Some("count")],
            ),
            (
                "tupleFixedRest",
                false,
                vec![(false, false, false), (false, true, false)],
                vec![None, None],
            ),
            (
                "tupleVariadic",
                false,
                vec![(false, false, false), (false, false, true)],
                vec![None, None],
            ),
            (
                "tupleEscapedLabel",
                false,
                vec![(false, false, false)],
                vec![Some("title")],
            ),
        ] {
            let answer = project.type_of(request(&fixture, marker, true, 2));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{config}/{marker}: {answer:?}");
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            let graph = result.description.expansion.unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            let TypeNodeData::Tuple {
                elements,
                readonly: actual_readonly,
                ..
            } = &root.data
            else {
                panic!("{config}/{marker}: {root:?}");
            };
            assert_eq!(*actual_readonly, readonly);
            assert_eq!(
                elements
                    .iter()
                    .map(|element| (element.optional, element.rest, element.variadic))
                    .collect::<Vec<_>>(),
                flags
            );
            assert_eq!(
                elements
                    .iter()
                    .map(|element| element.label.as_deref())
                    .collect::<Vec<_>>(),
                labels
            );
            let arguments = root.type_arguments.as_deref().unwrap_or(&[]);
            assert_eq!(arguments.len(), elements.len(), "{config}/{marker}");
            for (element, argument) in elements.iter().zip(arguments) {
                assert_eq!(
                    &element.value_type, argument,
                    "tuple arguments and elements share native identity"
                );
                let TypeRef::Ref { r#ref } = argument else {
                    panic!("{config}/{marker}: {argument:?}")
                };
                assert!(graph.nodes.iter().any(|node| node.id == *r#ref));
            }
            if marker == "tupleOptional" {
                let TypeRef::Ref { r#ref } = &elements[1].value_type else {
                    unreachable!()
                };
                let optional = graph.nodes.iter().find(|node| node.id == *r#ref).unwrap();
                assert_eq!(
                    matches!(optional.data, TypeNodeData::Union { .. }),
                    config == "tsconfig.json"
                );
            }
        }
    }
}

#[test]
fn a_single_rest_normalizes_to_the_actual_mutable_or_readonly_array() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, summary, readonly) in [
            ("tupleRestOnly", "number[]", false),
            ("tupleReadonlyRestOnly", "readonly number[]", true),
        ] {
            let answer = project.type_of(request(&fixture, marker, true, 2));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{config}/{marker}: {answer:?}")
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            assert_eq!(result.description.summary, summary);
            let graph = result.description.expansion.unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            let TypeNodeData::Array {
                element: TypeRef::Ref { r#ref },
                readonly: actual_readonly,
                ..
            } = &root.data
            else {
                panic!("{root:?}")
            };
            assert_eq!(*actual_readonly, readonly);
            assert_eq!(
                graph
                    .nodes
                    .iter()
                    .find(|node| node.id == *r#ref)
                    .unwrap()
                    .summary,
                "number"
            );
        }
    }
}

#[test]
fn a_consumed_error_element_is_invalid_even_when_native_tuple_root_is_healthy() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (expand, depth) in [(false, 0), (true, 0), (true, 2)] {
            let answer = project.type_of(request(&fixture, "tupleMissing", expand, depth));
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{config}: {answer:?}")
            };
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
            assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
            assert_eq!(error.path, Some(fixture.path("cases.ts")));
            assert_eq!(
                error.position,
                Some(position(SOURCE, SOURCE.find("MissingTuple").unwrap()))
            );
        }
    }
}

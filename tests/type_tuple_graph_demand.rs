//! A tuple header does not acquire its omitted deferred argument collection.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOmissionReason, TypeOutcome, TypeQuery, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/tuple-graph-demand/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/tuple-graph-demand/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/tuple-graph-demand/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/tuple-graph-demand/exact.json"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str, expand: bool, depth: usize) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
        },
        expand,
        depth,
    }
}

#[test]
fn omitted_tuple_arguments_preserve_healthy_alias_headers() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [
            ("hidden", "HiddenTuple"),
            ("generic", "GenericTuple<number>"),
            ("known", "KnownTuple"),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand, 0));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(result.description.summary, summary);
                assert!(result.issues.is_empty());
                if expand {
                    let graph = result.description.expansion.unwrap();
                    assert_eq!(graph.nodes.len(), 1);
                    assert!(graph.truncated);
                    let root = &graph.nodes[0];
                    assert!(root.type_arguments.is_none());
                    assert!(
                        matches!(&root.data, TypeNodeData::Tuple { elements, .. } if elements.is_empty())
                    );
                    let omitted = root.omitted.as_ref().unwrap();
                    assert_eq!(omitted.reason, TypeOmissionReason::Depth);
                    assert!(omitted.fields.iter().any(|field| field == "typeArguments"));
                    assert!(omitted.fields.iter().any(|field| field == "elements"));
                }
            }
        }
    }
}

#[test]
fn admitted_tuple_arguments_preserve_the_original_invalid_dependency() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, missing) in [("hidden", "MissingHidden"), ("generic", "MissingGeneric")] {
            let answer = project.type_of(request(&fixture, marker, true, 1));
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{config}/{marker}: {answer:?}");
            };
            assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
            assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
            let offset = SOURCE.find(missing).unwrap();
            assert_eq!(error.path, Some(fixture.path("cases.ts")));
            assert_eq!(error.position, Some(position(SOURCE, offset)));
        }
    }
}

#[test]
fn tuple_argument_collection_stays_lazy_at_exact_header_admission_boundaries() {
    // Public wire header = alias length + 504 UTF-8 bytes, including
    // omittedTypeArguments:2, omittedElements:2 and the qualified graph label. Replacing the former
    // count plus omission name with two minimal t0 references costs 3 bytes.
    // First tuple-element admission costs more, and the inventory header
    // needs 12 bytes. These follow the wire schema, not observed output.
    for (name_length, header_bytes, admitted) in [
        (261_639, 262_143, false),
        (261_638, 262_142, false),
        (261_637, 262_141, true),
        (261_636, 262_140, true),
    ] {
        let name = "A".repeat(name_length);
        let source = format!(
            "type {name} = [MissingGraph, number];\ninterface Array<T> {{ length: number; [index: number]: T }}\ninterface ReadonlyArray<T> {{ readonly length: number; readonly [index: number]: T }}\ndeclare const value: {name};\n"
        );
        for (strict, exact) in [(false, false), (true, false), (true, true)] {
            let config = format!(
                r#"{{"compilerOptions":{{"strict":{strict},"exactOptionalPropertyTypes":{exact},"noLib":true}},"files":["cases.ts"]}}"#
            );
            let fixture = Fixture::new(&[("cases.ts", &source), ("tsconfig.json", &config)]);
            let answer = fixture.load().type_of(TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path("cases.ts"),
                    position: position(&source, source.rfind("value:").unwrap()),
                },
                expand: true,
                depth: 1,
            });
            if admitted {
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!(
                        "{name_length}/{strict}/{exact}: admitted native argument failure was lost"
                    );
                };
                assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
                assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
                assert_eq!(error.path, Some(fixture.path("cases.ts")));
                assert_eq!(
                    error.position,
                    Some(position(&source, source.find("MissingGraph").unwrap()))
                );
            } else {
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!(
                        "{name_length}/{strict}/{exact}: elided argument collection was demanded: {answer:?}"
                    );
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert!(result.issues.is_empty());
                assert_eq!(
                    serde_json::to_vec(&result.description).unwrap().len(),
                    header_bytes
                );
                assert_eq!(result.description.summary, "…");
                assert!(result.description.summary_truncated);
                let graph = result.description.expansion.unwrap();
                assert_eq!(graph.nodes.len(), 1);
                assert!(graph.truncated);
                let root = &graph.nodes[0];
                assert_eq!(root.alias.as_ref().unwrap().name.len(), name_length);
                assert!(root.type_arguments.is_none());
                assert_eq!(root.omitted_type_arguments, Some(2));
                assert!(
                    matches!(&root.data, TypeNodeData::Tuple { elements, omitted_elements: Some(2), .. } if elements.is_empty())
                );
                let omitted = root.omitted.as_ref().unwrap();
                assert_eq!(omitted.reason, TypeOmissionReason::Bytes);
                assert_eq!(
                    omitted.fields,
                    [
                        "typeArguments",
                        "elements",
                        "properties",
                        "callSignatures",
                        "constructSignatures",
                        "indexSignatures"
                    ]
                );
            }
        }
    }
}

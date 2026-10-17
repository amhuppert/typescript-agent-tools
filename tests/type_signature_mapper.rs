//! Native signature substitution keeps actual local type identities and lazy questions.
mod support;

use std::path::Path;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::project::Project;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeExpansion, TypeNodeData, TypeOutcome, TypeQuery, TypeRef,
    TypeRequest, TypeResult, TypeSignature,
};

fn fixture() -> Fixture {
    Fixture::existing(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/native-signature-mapper"),
    )
}

fn query(fixture: &Fixture, name: &str) -> TypeQuery {
    let source = std::fs::read_to_string(fixture.path("cases.ts")).unwrap();
    let prefix = format!("declare let {name}:");
    let offset = source.find(&prefix).unwrap() + "declare let ".len();
    TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(&source, offset),
    }
}

fn selected(
    loaded: &Project,
    fixture: &Fixture,
    name: &str,
    expand: bool,
    depth: usize,
) -> TypeResult {
    let answer = loaded.type_of(TypeRequest {
        query: query(fixture, name),
        expand,
        depth,
    });
    let Ok(TypeAnswer::Selected(result)) = answer else {
        panic!("native signature question: {answer:?}");
    };
    assert_eq!(result.outcome, TypeOutcome::Found, "{result:?}");
    assert!(result.issues.is_empty(), "{result:?}");
    assert!(!result.description.summary_truncated);
    result
}

fn id(reference: &TypeRef) -> &str {
    let TypeRef::Ref { r#ref } = reference else {
        panic!("admitted native identity: {reference:?}");
    };
    r#ref
}

fn node<'a>(graph: &'a TypeExpansion, reference: &TypeRef) -> &'a TypeNodeData {
    &graph
        .nodes
        .iter()
        .find(|node| node.id == id(reference))
        .unwrap()
        .data
}

fn call(graph: &TypeExpansion) -> &TypeSignature {
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    let TypeNodeData::Function { members } = &root.data else {
        panic!("native callable shape: {root:?}");
    };
    &members.call_signatures.as_ref().unwrap()[0]
}

#[test]
fn actual_reference_mapper_substitutes_this_parameters_returns_and_constructs() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let loaded = fixture.load_config(config);
        let result = selected(&loaded, &fixture, "simple", true, 3);
        assert_eq!(result.description.summary, "Simple<string>");
        let graph = result.description.expansion.unwrap();
        assert!(!graph.truncated);
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .unwrap();
        let argument = &root.type_arguments.as_ref().unwrap()[0];
        assert_eq!(
            node(&graph, argument),
            &TypeNodeData::Primitive {
                name: PrimitiveName::String
            }
        );
        let TypeNodeData::Function { members } = &root.data else {
            panic!("native signature shape: {root:?}");
        };
        let call = &members.call_signatures.as_ref().unwrap()[0];
        let construct = &members.construct_signatures.as_ref().unwrap()[0];
        assert_eq!(call.this_parameter.as_ref().unwrap().value_type, *argument);
        for signature in [call, construct] {
            assert!(signature.type_parameters.is_empty());
            assert_eq!(signature.parameters.len(), 1);
            assert_eq!(signature.parameters[0].value_type, *argument);
            assert_eq!(signature.return_type, *argument);
        }
        assert_ne!(call.origins, construct.origins);
    }
}

#[test]
fn instantiated_local_parameters_retain_one_fresh_identity_per_signature() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let loaded = fixture.load_config(config);
        for (name, primitive) in [
            ("genericString", PrimitiveName::String),
            ("genericNumber", PrimitiveName::Number),
        ] {
            let result = selected(&loaded, &fixture, name, true, 3);
            let graph = result.description.expansion.unwrap();
            assert!(!graph.truncated);
            let signature = call(&graph);
            assert_eq!(signature.type_parameters.len(), 1);
            let local = &signature.parameters[0].value_type;
            assert_eq!(signature.return_type, *local);
            let TypeNodeData::TypeParameter {
                name,
                constraint,
                default,
            } = node(&graph, local)
            else {
                panic!("fresh native local parameter: {graph:?}");
            };
            assert_eq!(name, "U");
            for reference in [
                constraint.as_ref().unwrap(),
                default.as_ref().unwrap(),
                signature.type_parameters[0].constraint.as_ref().unwrap(),
                signature.type_parameters[0].default.as_ref().unwrap(),
            ] {
                assert_eq!(
                    node(&graph, reference),
                    &TypeNodeData::Primitive { name: primitive }
                );
            }
        }
    }
}

#[test]
fn cloned_local_mapper_composition_preserves_chains_and_shadowed_names() {
    let fixture = fixture();
    let loaded = fixture.load();
    let result = selected(&loaded, &fixture, "nested", true, 3);
    let graph = result.description.expansion.unwrap();
    assert!(!graph.truncated);
    let signature = call(&graph);
    assert_eq!(signature.type_parameters.len(), 2);
    let first = &signature.parameters[0].value_type;
    let second = &signature.parameters[1].value_type;
    assert_ne!(first, second);
    assert_eq!(signature.return_type, *second);
    let TypeNodeData::TypeParameter {
        name,
        constraint,
        default,
    } = node(&graph, first)
    else {
        panic!("first clone");
    };
    assert_eq!(name, "U");
    assert!(default.is_none());
    assert_eq!(
        node(&graph, constraint.as_ref().unwrap()),
        &TypeNodeData::Primitive {
            name: PrimitiveName::Number
        }
    );
    let TypeNodeData::TypeParameter {
        name,
        constraint,
        default,
    } = node(&graph, second)
    else {
        panic!("second clone");
    };
    assert_eq!(name, "V");
    assert_eq!(constraint.as_ref(), Some(first));
    assert_eq!(default.as_ref(), Some(first));
    let shadow = selected(&loaded, &fixture, "shadow", true, 3)
        .description
        .expansion
        .unwrap();
    let signature = call(&shadow);
    let TypeNodeData::TypeParameter {
        name,
        constraint,
        default,
    } = node(&shadow, &signature.parameters[0].value_type)
    else {
        panic!("shadow clone");
    };
    assert_eq!(name, "T");
    assert!(default.is_none());
    assert_eq!(
        node(&shadow, constraint.as_ref().unwrap()),
        &TypeNodeData::Primitive {
            name: PrimitiveName::String
        }
    );
}

#[test]
fn signature_clone_construction_leaves_unadmitted_invalid_constraints_and_defaults_lazy() {
    let fixture = fixture();
    let loaded = fixture.load();
    for expand in [false, true] {
        let result = selected(&loaded, &fixture, "lazy", expand, 0);
        assert_eq!(result.description.summary, "Lazy<string>");
        if expand {
            let graph = result.description.expansion.unwrap();
            assert!(graph.truncated);
            assert_eq!(graph.nodes.len(), 1);
        }
    }
    let answer = loaded.type_of(TypeRequest {
        query: query(&fixture, "lazy"),
        expand: true,
        depth: 3,
    });
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error))
        if error.kind == SemanticErrorKind::InvalidSemantic
            && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
        "admitted actual missing dependency must fail: {answer:?}"
    );
}

#[test]
fn optional_and_explicit_signature_unions_substitute_and_reduce_actual_alternatives() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let loaded = fixture.load_config(config);
        for name in ["optional", "unionString", "unionNumber"] {
            let result = selected(&loaded, &fixture, name, true, 3);
            let graph = result.description.expansion.unwrap();
            assert!(!graph.truncated);
            let signature = call(&graph);
            assert_eq!(signature.parameters[0].optional, name == "optional");
            let parameter = &signature.parameters[0].value_type;
            assert_eq!(signature.return_type, *parameter);
            if name == "unionNumber" || name == "optional" && config == "loose.json" {
                let name = if name == "unionNumber" {
                    PrimitiveName::Number
                } else {
                    PrimitiveName::String
                };
                assert_eq!(node(&graph, parameter), &TypeNodeData::Primitive { name });
            } else {
                let TypeNodeData::Union {
                    alternatives,
                    omitted_alternatives,
                } = node(&graph, parameter)
                else {
                    panic!("instantiated union: {graph:?}");
                };
                assert_eq!(*omitted_alternatives, None);
                assert_eq!(alternatives.len(), 2);
                let expected = if name == "optional" {
                    [
                        TypeNodeData::Undefined,
                        TypeNodeData::Primitive {
                            name: PrimitiveName::String,
                        },
                    ]
                } else {
                    [
                        TypeNodeData::Primitive {
                            name: PrimitiveName::String,
                        },
                        TypeNodeData::Primitive {
                            name: PrimitiveName::Number,
                        },
                    ]
                };
                for (reference, expected) in alternatives.iter().zip(expected) {
                    assert_eq!(node(&graph, reference), &expected);
                }
            }
        }
    }
}

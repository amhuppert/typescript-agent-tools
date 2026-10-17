//! Native type results exercise presentation and request policy end to end.
mod support;

use support::project;
use typescript_parser::analysis::{
    AnalysisError, QueryErrorKind, SemanticErrorKind, SemanticFailureReason,
};
use typescript_parser::references::Location;
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeCollection, TypeKind, TypeNodeData, TypeOmissionReason,
    TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

// Native proof: presentation-flow/cases.ts and its pinned oracle establish
// string | 0. The getter consumes an actually changed read, so this remains
// excluded by F even as conditionals and unchanged guard joins gain support.
const FLOW_DEPENDENCY: &str = include_str!("../fixtures/types/presentation-flow/dependency.ts");
fn flow_location(file: &str) -> Location {
    dependency_read_location(FLOW_DEPENDENCY, "input", file)
}
fn dependency_read_location(source: &str, name: &str, file: &str) -> Location {
    let start = source.find(&format!("return {name};")).unwrap() + "return ".len();
    Location {
        file: file.into(),
        start: support::position(source, start),
        end: support::position(source, start + name.len()),
    }
}
fn other_flow_dependency() -> String {
    FLOW_DEPENDENCY
        .replace("input", "laterInput")
        .replace("FlowSource", "LaterFlowSource")
        .replace("dependency", "laterDependency")
}
fn flow_index_globals() -> String {
    format!(
        "{FLOW_DEPENDENCY}\ninterface Array<T> {{ length: number; [index: number]: typeof dependency.value; }}\n"
    )
}

const FLOW_MESSAGE: &str =
    "native reference flow changes this read and requires excluded flow analysis";

// Calculate boundary inputs from the closed public wire shape, independently
// of the implementation's admission/accounting code or its observed output.
fn failed_property_wire(key_bytes: usize, array: bool) -> serde_json::Value {
    let key = "p".repeat(key_bytes);
    let (file, declaration, summary, failed_id, key_column) = if array {
        ("globals.ts", "interface Array<T>", "number[]", "t2", 22)
    } else {
        ("value.ts", "export interface Budget", "Budget", "t1", 27)
    };
    let line = FLOW_DEPENDENCY.lines().count() + 2;
    let graph_summary = if array {
        summary.to_owned()
    } else {
        format!("Budget ({file}:{line}:18)")
    };
    let mut root = serde_json::json!({
        "id":"t0", "summary":graph_summary, "summaryTruncated":false,
        "kind":if array { "array" } else { "object" },
        "properties":[{
            "key":{"kind":"string","name":key}, "kind":"property",
            "type":{"ref":failed_id}, "optional":false, "readonly":false,
            "accessibility":"public", "origins":[{"file":file,
                "start":{"line":line,"column":key_column},
                "end":{"line":line,"column":key_column+key_bytes}}]
        }], "callSignatures":[], "constructSignatures":[], "indexSignatures":[]
    });
    // Both source forms put their name at these exact columns.
    assert_eq!(format!("{declaration} {{ ").len() + 1, key_column);
    let mut nodes = vec![];
    if array {
        root["element"] = serde_json::json!({"ref":"t1"});
        root["typeArguments"] = serde_json::json!([{"ref":"t1"}]);
        root["readonly"] = serde_json::json!(false);
    }
    nodes.push(root);
    if array {
        nodes.push(serde_json::json!({"id":"t1","summary":"number","summaryTruncated":false,"kind":"primitive","name":"number"}));
    }
    nodes.push(serde_json::json!({"id":failed_id,"summary":"unsupported","summaryTruncated":false,
        "kind":"unsupported","reason":"flow_required","location":flow_location(file),"message":FLOW_MESSAGE}));
    serde_json::json!({"summary":summary,"summaryTruncated":false,
        "expansion":{"root":"t0","nodes":nodes,"truncated":false}})
}

fn blocked_array_wire(literal_bytes: usize) -> serde_json::Value {
    let issue = serde_json::json!({"reason":"flow_required","location":flow_location("globals.ts"),"message":FLOW_MESSAGE});
    let blocked: Vec<_> = [
        "properties",
        "callSignatures",
        "constructSignatures",
        "indexSignatures",
    ]
    .into_iter()
    .map(|field| serde_json::json!({"field":field,"issue":issue}))
    .collect();
    serde_json::json!({"summary":"…[]","summaryTruncated":true,"expansion":{
    "root":"t0","truncated":true,"nodes":[
        {"id":"t0","summary":"…[]","summaryTruncated":true,"typeArguments":[{"ref":"t1"}],
         "kind":"array","element":{"ref":"t1"},"readonly":false,"blockedCollections":blocked},
        {"id":"t1","summary":"…","summaryTruncated":true,"kind":"literal","base":"string","value":"p".repeat(literal_bytes)}
    ]}})
}

fn wire_boundary(complete_bytes: usize, wire: impl Fn(usize) -> serde_json::Value) -> usize {
    let baseline = 260_000;
    let key_bytes = baseline + complete_bytes - serde_json::to_vec(&wire(baseline)).unwrap().len();
    assert_eq!(
        serde_json::to_vec(&wire(key_bytes)).unwrap().len(),
        complete_bytes
    );
    key_bytes
}

fn array_fixture(merged: bool, strict: bool) -> support::Fixture {
    let config = format!(
        r#"{{"compilerOptions":{{"strict":{strict},"noLib":true,"resolveJsonModule":true,"module":"esnext","moduleResolution":"bundler"}},"files":["globals.ts","consumer.ts"]}}"#
    );
    let globals = if merged {
        include_str!("../fixtures/types/presentation-array/merged/globals.ts")
    } else {
        include_str!("../fixtures/types/presentation-array/invalid-index/globals.ts")
    };
    support::Fixture::new(&[
        ("tsconfig.json", &config),
        ("globals.ts", globals),
        (
            "data.json",
            include_str!("../fixtures/types/presentation-array/merged/data.json"),
        ),
        (
            "consumer.ts",
            include_str!("../fixtures/types/presentation-array/merged/consumer.ts"),
        ),
    ])
}

fn array_request(fixture: &support::Fixture, expand: bool, depth: usize) -> TypeRequest {
    let text = include_str!("../fixtures/types/presentation-array/merged/consumer.ts");
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("consumer.ts"),
            position: SourcePosition {
                line: 1,
                column: text.find("*/values").unwrap() + 3,
            },
        },
        expand,
        depth,
    }
}

fn reference_shape_request(
    fixture: &support::Fixture,
    name: &str,
    expand: bool,
    depth: usize,
) -> TypeRequest {
    let source = include_str!("../fixtures/types/reference-shape/cases.ts");
    let anchor = format!("let {name}:");
    let offset = source.find(&anchor).unwrap() + "let ".len();
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("cases.ts"),
            position: support::position(source, offset),
        },
        expand,
        depth,
    }
}

#[test]
fn ordinary_reference_kind_uses_actual_instantiated_call_or_construct_structure() {
    for strict in [false, true] {
        let config = format!(
            r#"{{"compilerOptions":{{"strict":{strict},"noLib":true}},"files":["cases.ts"]}}"#
        );
        let fixture = support::Fixture::new(&[
            ("tsconfig.json", &config),
            (
                "cases.ts",
                include_str!("../fixtures/types/reference-shape/cases.ts"),
            ),
        ]);
        let loaded = fixture.load();
        for (name, label, callable, constructable) in [
            ("object", "Box<string>", false, false),
            ("callable", "Callable<string>", true, false),
            ("constructable", "Constructable<string>", false, true),
        ] {
            for depth in [0, 1, 2] {
                let answer = loaded.type_of(reference_shape_request(&fixture, name, true, depth));
                assert!(
                    matches!(&answer, Ok(TypeAnswer::Selected(_))),
                    "real reference shape must be established, not guessed from its name: {name}/{depth}: {answer:?}"
                );
                let TypeAnswer::Selected(result) = answer.unwrap() else {
                    unreachable!()
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert!(result.issues.is_empty());
                assert_eq!(result.description.summary, label);
                let graph = result.description.expansion.unwrap();
                let root = &graph.nodes[0];
                let members = match &root.data {
                    TypeNodeData::Object { members } if !callable && !constructable => members,
                    TypeNodeData::Function { members } if callable || constructable => members,
                    _ => {
                        panic!("actual callable/constructable structure determines kind: {root:?}")
                    }
                };
                if depth == 0 {
                    assert!(graph.truncated);
                    assert_eq!(graph.nodes.len(), 1);
                    assert_eq!(members.properties.is_none(), !callable && !constructable);
                    assert_eq!(members.call_signatures.is_none(), callable);
                    assert_eq!(members.construct_signatures.is_none(), constructable);
                    assert_eq!(members.omitted_properties, None);
                    assert_eq!(members.omitted_call_signatures, None);
                    assert_eq!(members.omitted_construct_signatures, None);
                    continue;
                }
                assert!(!graph.truncated);
                assert_eq!(graph.nodes.len(), 2);
                let TypeRef::Ref { r#ref: argument } = &root.type_arguments.as_ref().unwrap()[0]
                else {
                    panic!("actual type argument is an admitted shared identity")
                };
                assert_eq!(
                    graph
                        .nodes
                        .iter()
                        .find(|node| node.id == *argument)
                        .unwrap()
                        .data,
                    TypeNodeData::Primitive {
                        name: PrimitiveName::String
                    }
                );
                if callable || constructable {
                    let signatures = if callable {
                        members.call_signatures.as_ref().unwrap()
                    } else {
                        members.construct_signatures.as_ref().unwrap()
                    };
                    assert_eq!(signatures.len(), 1);
                    assert_eq!(signatures[0].parameters.len(), 1);
                    assert_eq!(
                        signatures[0].parameters[0].value_type,
                        TypeRef::Ref {
                            r#ref: argument.clone()
                        }
                    );
                    assert_eq!(
                        signatures[0].return_type,
                        TypeRef::Ref {
                            r#ref: argument.clone()
                        }
                    );
                } else {
                    let properties = members.properties.as_ref().unwrap();
                    assert_eq!(properties.len(), 1);
                    assert_eq!(
                        properties[0].read_type,
                        TypeRef::Ref {
                            r#ref: argument.clone()
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn ordinary_reference_depth_zero_keeps_property_values_lazy_after_required_kind_inventory() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/reference-shape");
    let fixture = support::Fixture::existing(&directory);
    for config in ["tsconfig.json", "loose.json"] {
        let loaded = fixture.load_config(config);
        for expand in [false, true] {
            let answer = loaded.type_of(reference_shape_request(
                &fixture,
                "missingProperty",
                expand,
                0,
            ));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "required kind inventory cannot eagerly demand ordinary property values: {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, "MissingProperty<string>");
            assert!(result.issues.is_empty());
        }
        let answer = loaded.type_of(reference_shape_request(
            &fixture,
            "missingProperty",
            true,
            1,
        ));
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error))
            if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
            "admitted property recovery fails the final type operation: {answer:?}"
        );
    }
}

#[test]
fn ordinary_reference_real_kind_consumes_invalid_index_inventory_even_at_depth_zero() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/reference-shape");
    let fixture = support::Fixture::existing(&directory);
    for config in ["tsconfig.json", "loose.json"] {
        let loaded = fixture.load_config(config);
        let answer = loaded.type_of(reference_shape_request(&fixture, "missingIndex", false, 0));
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "nominal summary does not ask inventory: {answer:?}"
        );
        for depth in [0, 1, 2] {
            let answer = loaded.type_of(reference_shape_request(
                &fixture,
                "missingIndex",
                true,
                depth,
            ));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
                "required actual-kind inventory recovery cannot be hidden by display depth: {depth}: {answer:?}"
            );
            let Err(AnalysisError::Semantic(error)) = answer else {
                unreachable!()
            };
            let source = include_str!("../fixtures/types/reference-shape/cases.ts");
            let offset =
                source.find("[key: string]: MissingType").unwrap() + "[key: string]: ".len();
            assert_eq!(error.path, Some(fixture.path("cases.ts")));
            assert_eq!(error.position, Some(support::position(source, offset)));
        }
    }
}

#[test]
fn unsupported_required_kind_of_an_admitted_reference_child_preserves_the_established_parent() {
    let source = format!(
        "{FLOW_DEPENDENCY}\ninterface Child {{ (value: string): string; [key: string]: typeof dependency.value }}\nexport interface Host {{ nested: Child }}\n"
    );
    let fixture = project(&[
        ("value.ts", &source),
        ("consumer.ts", "import { Host } from './value';\n"),
    ]);
    let loaded = fixture.load();
    for depth in [0, 1] {
        let answer = loaded.type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("consumer.ts"),
                position: SourcePosition {
                    line: 1,
                    column: 10,
                },
            },
            expand: true,
            depth,
        });
        assert!(matches!(&answer, Ok(TypeAnswer::Selected(_))), "{answer:?}");
        if depth == 0 {
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            continue;
        }
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.outcome, TypeOutcome::Partial);
        assert_eq!(result.issues.len(), 1);
        let issue = &result.issues[0];
        let graph = result.description.expansion.unwrap();
        let TypeNodeData::Object { members } = &graph.nodes[0].data else {
            panic!("{graph:?}")
        };
        let TypeRef::Ref { r#ref } = &members.properties.as_ref().unwrap()[0].read_type else {
            panic!("admitted failure must receive a bounded failure record")
        };
        let failed = graph.nodes.iter().find(|node| node.id == *r#ref).unwrap();
        let TypeNodeData::Unsupported {
            reason,
            location,
            message,
        } = &failed.data
        else {
            panic!("a failed inventory is not a guessed object/function: {failed:?}")
        };
        assert_eq!(reason, &issue.reason);
        assert_eq!(location, &issue.location);
        assert_eq!(message, &issue.message);
        assert_eq!(failed.alias, None);
        assert_eq!(failed.type_arguments, None);
    }
}

#[test]
fn array_summary_and_depth_zero_do_not_demand_unknown_or_invalid_member_inventory() {
    // Fresh pinned roots are genuine healthy number[] in both families. A
    // separate native index question consumes MissingType only in the latter.
    for merged in [false, true] {
        for strict in [false, true] {
            let fixture = array_fixture(merged, strict);
            let project = fixture.load();
            for expand in [false, true] {
                let answer = project.type_of(array_request(&fixture, expand, 0));
                assert!(
                    matches!(&answer, Ok(TypeAnswer::Selected(_))),
                    "undemanded inventory cannot invalidate established Array: {answer:?}"
                );
                let TypeAnswer::Selected(result) = answer.unwrap() else {
                    unreachable!()
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert!(result.issues.is_empty());
                assert_eq!(result.description.summary, "number[]");
                assert!(!result.description.summary_truncated);
                if !expand {
                    assert!(result.description.expansion.is_none());
                    continue;
                }
                let graph = result.description.expansion.unwrap();
                assert_eq!(graph.nodes.len(), 1);
                assert!(graph.truncated);
                let root = &graph.nodes[0];
                let TypeNodeData::Array {
                    element,
                    readonly,
                    members,
                } = &root.data
                else {
                    panic!("real Array kind: {root:?}")
                };
                assert!(!readonly);
                assert_eq!(
                    *element,
                    TypeRef::Omitted {
                        omitted: TypeOmissionReason::Depth,
                        kind: TypeKind::Primitive,
                        summary: "number".into(),
                        summary_truncated: false
                    }
                );
                assert!(
                    members.properties.is_none()
                        && members.call_signatures.is_none()
                        && members.construct_signatures.is_none()
                        && members.index_signatures.is_none()
                );
                assert!(members.blocked_collections.is_none());
                assert!(
                    members.omitted_properties.is_none()
                        && members.omitted_call_signatures.is_none()
                        && members.omitted_construct_signatures.is_none()
                        && members.omitted_index_signatures.is_none()
                );
                let omitted = root.omitted.as_ref().unwrap();
                assert_eq!(omitted.reason, TypeOmissionReason::Depth);
                assert_eq!(
                    omitted.fields,
                    [
                        "typeArguments",
                        "element",
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

#[test]
fn unchanged_flow_join_establishes_the_complete_array_inventory() {
    for strict in [false, true] {
        let fixture = annotated_array_fixture(
            include_str!("../fixtures/types/presentation-array/flow-index/globals.ts"),
            "export let current: Array<number>;\n",
            strict,
        );
        let answer = fixture.load().type_of(annotated_array_request(&fixture, 1));
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!("{answer:?}");
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        let TypeNodeData::Array { members, .. } = &graph.nodes[0].data else {
            panic!("{graph:?}");
        };
        assert!(members.blocked_collections.is_none());
        assert_eq!(members.index_signatures.as_ref().unwrap().len(), 1);
    }
}

#[test]
fn demanded_unsupported_array_inventory_is_partial_and_blocked_without_guessed_counts() {
    for strict in [false, true] {
        let fixture = annotated_array_fixture(
            &flow_index_globals(),
            "export let current: Array<number>;\n",
            strict,
        );
        let answer = fixture.load().type_of(annotated_array_request(&fixture, 1));
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "established Array retains its blocked inventory: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.outcome, TypeOutcome::Partial);
        assert_eq!(result.description.summary, "number[]");
        assert_eq!(result.issues.len(), 1);
        assert_eq!(result.issues[0].reason, SemanticFailureReason::FlowRequired);
        assert_eq!(result.issues[0].location, flow_location("globals.ts"));
        let graph = result.description.expansion.unwrap();
        assert!(
            !graph.truncated,
            "semantic blocked enumeration alone is not display elision"
        );
        assert_eq!(graph.nodes.len(), 2);
        let root = &graph.nodes[0];
        let TypeNodeData::Array {
            element, members, ..
        } = &root.data
        else {
            panic!("established Array")
        };
        assert_eq!(root.type_arguments, Some(vec![element.clone()]));
        assert!(root.omitted.is_none());
        assert!(
            members.properties.is_none()
                && members.call_signatures.is_none()
                && members.construct_signatures.is_none()
                && members.index_signatures.is_none()
        );
        assert!(
            members.omitted_properties.is_none()
                && members.omitted_call_signatures.is_none()
                && members.omitted_construct_signatures.is_none()
                && members.omitted_index_signatures.is_none()
        );
        let blocked = members.blocked_collections.as_ref().unwrap();
        assert_eq!(
            blocked.iter().map(|row| row.field).collect::<Vec<_>>(),
            [
                TypeCollection::Properties,
                TypeCollection::CallSignatures,
                TypeCollection::ConstructSignatures,
                TypeCollection::IndexSignatures
            ]
        );
        assert!(blocked.iter().all(|row| row.issue == result.issues[0]));
    }
}

#[test]
fn demanded_invalid_array_index_dependency_remains_a_whole_failure() {
    for strict in [false, true] {
        let fixture = array_fixture(false, strict);
        let answer = fixture.load().type_of(array_request(&fixture, true, 1));
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic
            && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
            "consumed index recovery cannot become partial or healthy Array: {answer:?}"
        );
        let Err(AnalysisError::Semantic(error)) = answer else {
            unreachable!()
        };
        assert_eq!(
            error.path,
            Some(std::fs::canonicalize(fixture.path("globals.ts")).unwrap())
        );
        assert_eq!(
            error.position,
            Some(SourcePosition {
                line: 1,
                column: 39
            })
        );
    }
}

fn annotated_array_fixture(globals: &str, value: &str, strict: bool) -> support::Fixture {
    let config = format!(
        r#"{{"compilerOptions":{{"strict":{strict},"noLib":true}},"files":["globals.ts","consumer.ts"]}}"#
    );
    support::Fixture::new(&[
        ("tsconfig.json", &config),
        ("globals.ts", globals),
        ("value.ts", value),
        ("consumer.ts", "import { current } from './value';\n"),
    ])
}

fn annotated_array_request(fixture: &support::Fixture, depth: usize) -> TypeRequest {
    TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("consumer.ts"),
            position: SourcePosition {
                line: 1,
                column: 10,
            },
        },
        expand: true,
        depth,
    }
}

#[test]
fn actual_array_graph_shares_union_arguments_at_depth_and_node_boundaries() {
    // Fourteen fresh native sessions pin the three valid shallow union sizes
    // and both null profiles in presentation-array/evidence/limit-observations.
    // Array + union argument + numeric index key consume three graph records.
    for count in [1_996, 1_997, 1_998] {
        let alternatives = (1..=count)
            .map(|number| number.to_string())
            .collect::<Vec<_>>()
            .join(" | ");
        let value = format!("export let current: Array<{alternatives}>;\n");
        for strict in [false, true] {
            let fixture = annotated_array_fixture(
                "interface Array<T> { [index: number]: T; }\n",
                &value,
                strict,
            );
            let project = fixture.load();
            let mut full = None;
            for depth in [1, 2, 3] {
                let answer = project.type_of(annotated_array_request(&fixture, depth));
                assert!(
                    matches!(&answer, Ok(TypeAnswer::Selected(_))),
                    "valid shallow Array union cannot become a semantic limit failure: {answer:?}"
                );
                let TypeAnswer::Selected(result) = answer.unwrap() else {
                    unreachable!()
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert!(result.issues.is_empty());
                let graph = result.description.expansion.as_ref().unwrap();
                let root = &graph.nodes[0];
                let TypeNodeData::Array {
                    element, members, ..
                } = &root.data
                else {
                    panic!("actual Array")
                };
                assert_eq!(root.type_arguments, Some(vec![element.clone()]));
                assert_eq!(
                    members.index_signatures.as_ref().unwrap()[0].value_type,
                    *element
                );
                assert_eq!(
                    graph.nodes[2].data,
                    TypeNodeData::Primitive {
                        name: PrimitiveName::Number
                    }
                );
                let TypeNodeData::Union {
                    alternatives,
                    omitted_alternatives,
                } = &graph.nodes[1].data
                else {
                    panic!("shared actual argument union")
                };
                if depth == 1 {
                    assert_eq!(graph.nodes.len(), 3);
                    assert!(alternatives.is_empty());
                    assert_eq!(*omitted_alternatives, Some(count));
                    assert_eq!(
                        graph.nodes[1].omitted.as_ref().unwrap().reason,
                        TypeOmissionReason::Depth
                    );
                } else {
                    assert_eq!(graph.nodes.len(), (count + 3).min(2_000));
                    assert_eq!(alternatives.len(), count);
                    assert_eq!(*omitted_alternatives, None);
                    assert!(graph.nodes[1].omitted.is_none());
                    if count + 3 <= 2_000 {
                        assert!(
                            alternatives
                                .iter()
                                .all(|edge| matches!(edge, TypeRef::Ref { .. }))
                        );
                    } else {
                        assert!(
                            alternatives[..count - 1]
                                .iter()
                                .all(|edge| matches!(edge, TypeRef::Ref { .. }))
                        );
                        assert_eq!(
                            alternatives[count - 1],
                            TypeRef::Omitted {
                                omitted: TypeOmissionReason::Nodes,
                                kind: TypeKind::Literal,
                                summary: count.to_string(),
                                summary_truncated: false
                            }
                        );
                    }
                    if depth == 2 {
                        full = Some(result.description.clone());
                    } else {
                        assert_eq!(
                            Some(result.description),
                            full,
                            "extra depth cannot rediscover a shared argument more deeply"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn array_failed_child_preserves_partial_at_exact_compact_byte_boundaries() {
    // Pinned native roots are number[] and each lazy property is string | 0.
    // The excluded flow read is demanded only after property metadata fits.
    for complete_bytes in [262_143, 262_144, 262_145] {
        let key_bytes = wire_boundary(complete_bytes, |bytes| failed_property_wire(bytes, true));
        let key = "p".repeat(key_bytes);
        let globals = format!(
            "{FLOW_DEPENDENCY}\ninterface Array<T> {{ {key}: typeof dependency.value; }}\n"
        );
        for strict in [false, true] {
            let fixture =
                annotated_array_fixture(&globals, "export let current: Array<number>;\n", strict);
            let answer = fixture.load().type_of(annotated_array_request(&fixture, 1));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "encountered Array property failure must survive display rollback: {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.outcome, TypeOutcome::Partial);
            assert_eq!(result.issues.len(), 1);
            let issue = &result.issues[0];
            assert_eq!(issue.reason, SemanticFailureReason::FlowRequired);
            assert_eq!(issue.location, flow_location("globals.ts"));
            let bytes = serde_json::to_vec(&result.description).unwrap().len();
            let graph = result.description.expansion.as_ref().unwrap();
            let TypeNodeData::Array {
                members, element, ..
            } = &graph.nodes[0].data
            else {
                panic!("real Array")
            };
            assert_eq!(graph.nodes[0].type_arguments, Some(vec![element.clone()]));
            if complete_bytes <= 262_144 {
                assert_eq!(bytes, complete_bytes);
                assert_eq!(
                    serde_json::to_value(&result.description).unwrap(),
                    failed_property_wire(key_bytes, true)
                );
                assert_eq!(graph.nodes.len(), 3);
                assert!(!graph.truncated);
                assert_eq!(members.omitted_properties, None);
                assert_eq!(
                    members.properties.as_ref().unwrap()[0].key,
                    typescript_parser::types::TypeKey::String { name: key.clone() }
                );
                assert!(
                    matches!(&graph.nodes[2].data, TypeNodeData::Unsupported { location, message, .. }
                    if location == &issue.location && message == &issue.message)
                );
            } else {
                assert!(bytes < 262_144);
                assert_eq!(
                    graph.nodes.len(),
                    2,
                    "rollback retains the previously admitted shared element"
                );
                assert!(graph.truncated);
                assert!(members.properties.as_ref().unwrap().is_empty());
                assert_eq!(members.omitted_properties, Some(1));
            }
        }
    }
}

#[test]
fn array_property_metadata_avoids_an_unshown_invalid_child_question() {
    let key = "p".repeat(263_000);
    let globals = format!("interface Array<T> {{ {key}: MissingType; }}\n");
    for strict in [false, true] {
        let fixture =
            annotated_array_fixture(&globals, "export let current: Array<number>;\n", strict);
        let answer = fixture.load().type_of(annotated_array_request(&fixture, 1));
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "an unadmitted oversized property cannot consume its native child recovery: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.truncated);
        let TypeNodeData::Array { members, .. } = &graph.nodes[0].data else {
            panic!("real Array")
        };
        assert!(members.properties.as_ref().unwrap().is_empty());
        assert_eq!(members.omitted_properties, Some(1));
    }
}

#[test]
fn blocked_array_inventory_retains_its_issue_when_explanation_exceeds_byte_budget() {
    // Fresh native sessions establish healthy Array roots and an index
    // annotation that consumes excluded site-sensitive flow. Independent closed-schema
    // encoding places the complete four explanations at limit-1/limit/limit+1.
    for complete_bytes in [262_143, 262_144, 262_145] {
        let literal_bytes = wire_boundary(complete_bytes, blocked_array_wire);
        let payload = "p".repeat(literal_bytes);
        let value = format!("export let current: Array<\"{payload}\">;\n");
        for strict in [false, true] {
            let fixture = annotated_array_fixture(&flow_index_globals(), &value, strict);
            let answer = fixture.load().type_of(annotated_array_request(&fixture, 1));
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "encountered whole inventory failure cannot become a whole display error: {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.outcome, TypeOutcome::Partial);
            assert_eq!(result.issues.len(), 1);
            let issue = &result.issues[0];
            assert_eq!(issue.reason, SemanticFailureReason::FlowRequired);
            assert_eq!(issue.location, flow_location("globals.ts"));
            assert_eq!(result.description.summary, "…[]");
            assert!(result.description.summary_truncated);
            let bytes = serde_json::to_vec(&result.description).unwrap().len();
            let graph = result.description.expansion.as_ref().unwrap();
            assert!(
                graph.truncated,
                "actual summary elision counts independently of semantic blockage"
            );
            assert_eq!(graph.nodes.len(), 2);
            let root = &graph.nodes[0];
            let TypeNodeData::Array {
                members, element, ..
            } = &root.data
            else {
                panic!("actual Array")
            };
            assert_eq!(root.type_arguments, Some(vec![element.clone()]));
            assert_eq!(
                graph.nodes[1].data,
                TypeNodeData::Literal {
                    base: PrimitiveName::String,
                    value: payload.clone().into()
                }
            );
            assert!(
                members.properties.is_none()
                    && members.call_signatures.is_none()
                    && members.construct_signatures.is_none()
                    && members.index_signatures.is_none()
            );
            assert!(
                members.omitted_properties.is_none()
                    && members.omitted_call_signatures.is_none()
                    && members.omitted_construct_signatures.is_none()
                    && members.omitted_index_signatures.is_none()
            );
            if complete_bytes <= 262_144 {
                assert_eq!(bytes, complete_bytes);
                assert_eq!(
                    serde_json::to_value(&result.description).unwrap(),
                    blocked_array_wire(literal_bytes)
                );
                assert!(root.omitted.is_none());
                let blocked = members.blocked_collections.as_ref().unwrap();
                assert_eq!(blocked.len(), 4);
                assert!(blocked.iter().all(|row| row.issue == *issue));
            } else {
                assert!(bytes < 262_144);
                assert!(members.blocked_collections.is_none());
                let omitted = root.omitted.as_ref().unwrap();
                assert_eq!(omitted.reason, TypeOmissionReason::Bytes);
                assert_eq!(
                    omitted.fields,
                    [
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

#[test]
fn native_nonprimitive_object_flags_order_before_structured_union_members() {
    for strict in [true, false] {
        let config = format!(r#"{{"compilerOptions":{{"strict":{strict},"noLib":true}}}}"#);
        let fixture = support::Fixture::new(&[
            ("tsconfig.json", &config),
            (
                "value.ts",
                "export let current: object | { value: string };\n",
            ),
            ("consumer.ts", "import { current } from './value';\n"),
        ]);
        let answer = fixture.load().type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("consumer.ts"),
                position: SourcePosition {
                    line: 1,
                    column: 10,
                },
            },
            expand: true,
            depth: 3,
        });
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "native object union must remain valid: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
        assert_eq!(result.description.summary, "object | { value: string; }");
        let graph = result.description.expansion.unwrap();
        assert_eq!(
            graph.nodes[1].data,
            TypeNodeData::Primitive {
                name: PrimitiveName::Object
            }
        );
        assert!(matches!(graph.nodes[2].data, TypeNodeData::Object { .. }));
    }
}

#[test]
fn invalid_type_depth_fails_before_target_selection_or_semantic_demand() {
    let fixture = project(&[("value.ts", "export let value = 1;\n")]);
    let loaded = fixture.load();
    for depth in [33, usize::MAX] {
        for expand in [false, true] {
            for query in [
                TypeQuery::Name {
                    name: "missing".into(),
                    file: None,
                },
                TypeQuery::At {
                    file: fixture.path("outside.ts"),
                    position: SourcePosition { line: 1, column: 1 },
                },
            ] {
                let answer = loaded.type_of(TypeRequest {
                    query,
                    expand,
                    depth,
                });
                assert!(
                    matches!(&answer, Err(AnalysisError::Query(error)) if error.kind == QueryErrorKind::InvalidRequest),
                    "invalid display depth must fail before target selection: {answer:?}"
                );
                let Err(AnalysisError::Query(error)) = answer else {
                    unreachable!()
                };
                assert_eq!(
                    error.message,
                    format!("type expansion depth must be between 0 and 32, got {depth}")
                );
                assert_eq!(error.path, None);
                assert_eq!(error.position, None);
            }
        }
    }
}

#[test]
fn issues_sort_by_exact_source_site_without_reordering_graph_members() {
    let earlier = format!("{FLOW_DEPENDENCY}export type Earlier = typeof dependency.value;\n");
    let later = format!(
        "{}export type Later = typeof laterDependency.value;\n",
        other_flow_dependency()
    );
    let combined = format!("{earlier}{later}");
    for (early_module, late_module, expected) in [
        (
            "./same",
            "./same",
            vec![
                dependency_read_location(&combined, "input", "same.ts"),
                dependency_read_location(&combined, "laterInput", "same.ts"),
            ],
        ),
        (
            "./A",
            "./z",
            vec![
                flow_location("A.ts"),
                dependency_read_location(&later, "laterInput", "z.ts"),
            ],
        ),
    ] {
        let value = format!(
            "import {{ Earlier }} from '{early_module}';\nimport {{ Later }} from '{late_module}';\nexport interface Sorted {{ later: Later; earlier: Earlier; }}\n"
        );
        let fixture = project(&[
            ("same.ts", &combined),
            ("A.ts", &earlier),
            ("z.ts", &later),
            ("value.ts", &value),
            ("consumer.ts", "import { Sorted } from './value';\n"),
        ]);
        let answer = fixture.load().type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("consumer.ts"),
                position: SourcePosition {
                    line: 1,
                    column: 10,
                },
            },
            expand: true,
            depth: 1,
        });
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "established reverse-ordered members must keep their partial details: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.outcome, TypeOutcome::Partial);
        let actual: Vec<_> = result
            .issues
            .iter()
            .map(|issue| issue.location.clone())
            .collect();
        assert_eq!(
            actual, expected,
            "issues use exact source-site order, not demand encounter order"
        );
        let graph = result.description.expansion.unwrap();
        let TypeNodeData::Object { members } = &graph.nodes[0].data else {
            panic!("the established root remains an object")
        };
        let properties = members.properties.as_ref().unwrap();
        assert_eq!(
            properties
                .iter()
                .map(|property| &property.key)
                .collect::<Vec<_>>(),
            vec![
                &typescript_parser::types::TypeKey::String {
                    name: "later".into()
                },
                &typescript_parser::types::TypeKey::String {
                    name: "earlier".into()
                }
            ]
        );
        assert!(
            !graph.truncated,
            "semantic issue ordering does not elide display content"
        );
    }
}

#[test]
fn encountered_lazy_failure_remains_partial_when_its_display_record_exceeds_bytes() {
    // The closed schema fixes the exact boundary; native evidence fixes the
    // healthy root and the changed-flow dependency behind its lazy property.
    for complete_bytes in [262_143, 262_144, 262_145] {
        let key_bytes = wire_boundary(complete_bytes, |bytes| failed_property_wire(bytes, false));
        let key = "p".repeat(key_bytes);
        let value = format!(
            "{FLOW_DEPENDENCY}\nexport interface Budget {{ {key}: typeof dependency.value; }}\n"
        );
        let fixture = project(&[
            ("value.ts", &value),
            ("consumer.ts", "import { Budget } from './value';\n"),
        ]);
        let answer = fixture.load().type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("consumer.ts"),
                position: SourcePosition {
                    line: 1,
                    column: 10,
                },
            },
            expand: true,
            depth: 1,
        });
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "an encountered child failure must survive display rollback: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.outcome, TypeOutcome::Partial);
        assert_eq!(result.issues.len(), 1);
        let issue = &result.issues[0];
        assert_eq!(issue.reason, SemanticFailureReason::FlowRequired);
        assert_eq!(issue.location, flow_location("value.ts"));
        let encoded = serde_json::to_vec(&result.description).unwrap().len();
        let graph = result.description.expansion.as_ref().unwrap();
        let TypeNodeData::Object { members } = &graph.nodes[0].data else {
            panic!("a lazy root remains established")
        };
        if complete_bytes <= 262_144 {
            assert_eq!(encoded, complete_bytes);
            assert_eq!(
                serde_json::to_value(&result.description).unwrap(),
                failed_property_wire(key_bytes, false)
            );
            assert_eq!(graph.nodes.len(), 2);
            assert!(!graph.truncated);
            let property = &members.properties.as_ref().unwrap()[0];
            assert_eq!(
                property.key,
                typescript_parser::types::TypeKey::String { name: key }
            );
            assert!(
                matches!(&graph.nodes[1].data, TypeNodeData::Unsupported { location, message, .. }
                if location == &issue.location && message == &issue.message)
            );
            assert_eq!(members.omitted_properties, None);
        } else {
            assert!(encoded < 262_144);
            assert_eq!(
                graph.nodes.len(),
                1,
                "the staged failed display node must roll back with its parent row"
            );
            assert!(graph.truncated);
            assert!(members.properties.as_ref().unwrap().is_empty());
            assert_eq!(members.omitted_properties, Some(1));
        }
    }
}

#[test]
fn property_failure_edges_distinguish_shared_reads_from_independent_accessor_writes() {
    for (declaration, failures) in [
        ("value: typeof dependency.value;", 1),
        (
            "get value(): typeof dependency.value; set value(input: typeof dependency.value);",
            1,
        ),
        (
            "get value(): typeof dependency.value; set value(input: typeof laterDependency.value);",
            2,
        ),
    ] {
        let other = other_flow_dependency();
        let value =
            format!("{FLOW_DEPENDENCY}{other}export interface Budget {{ {declaration} }}\n");
        let fixture = project(&[
            ("value.ts", &value),
            ("consumer.ts", "import { Budget } from './value';\n"),
        ]);
        let answer = fixture.load().type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("consumer.ts"),
                position: SourcePosition {
                    line: 1,
                    column: 10,
                },
            },
            expand: true,
            depth: 1,
        });
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!("{answer:?}");
        };
        assert_eq!(result.outcome, TypeOutcome::Partial);
        assert_eq!(result.issues.len(), failures);
        assert_eq!(result.issues[0].location, flow_location("value.ts"));
        assert!(
            result
                .issues
                .iter()
                .all(|issue| issue.reason == SemanticFailureReason::FlowRequired)
        );
        if failures == 2 {
            assert_eq!(
                result.issues[1].location,
                dependency_read_location(&value, "laterInput", "value.ts")
            );
        }
        let graph = result.description.expansion.unwrap();
        assert_eq!(graph.nodes.len(), 1 + failures);
        let TypeNodeData::Object { members } = &graph.nodes[0].data else {
            panic!("established interface root");
        };
        let property = &members.properties.as_ref().unwrap()[0];
        assert_eq!(property.read_type, TypeRef::Ref { r#ref: "t1".into() });
        assert_eq!(
            property.write_type,
            if failures == 2 {
                Some(TypeRef::Ref { r#ref: "t2".into() })
            } else {
                None
            }
        );
        for (node, issue) in graph.nodes[1..].iter().zip(&result.issues) {
            assert!(
                matches!(&node.data, TypeNodeData::Unsupported { location, .. } if *location == issue.location)
            );
        }
        assert!(!graph.truncated);
    }
}

#[test]
fn oversized_member_metadata_avoids_an_unshown_invalid_child_demand() {
    let key = "p".repeat(263_000);
    let value = format!("export interface Budget {{ {key}: MissingType; }}\n");
    let fixture = project(&[
        ("value.ts", &value),
        ("consumer.ts", "import { Budget } from './value';\n"),
    ]);
    let answer = fixture.load().type_of(TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("consumer.ts"),
            position: SourcePosition {
                line: 1,
                column: 10,
            },
        },
        expand: true,
        depth: 1,
    });
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(_))),
        "metadata admission must exclude the unshown member before checking its unresolved annotation: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert!(result.issues.is_empty());
    assert_eq!(result.description.summary, "Budget");
    let graph = result.description.expansion.unwrap();
    assert_eq!(graph.nodes.len(), 1);
    assert!(graph.truncated);
    let TypeNodeData::Object { members } = &graph.nodes[0].data else {
        panic!("the admitted root stays an object")
    };
    assert!(members.properties.as_ref().unwrap().is_empty());
    assert_eq!(members.omitted_properties, Some(1));
}

#[test]
fn scalars_are_complete_at_zero_depth_and_maximum_valid_depth_is_admitted() {
    let imports = "import { text, mutable } from './value';\n";
    let fixture = project(&[
        (
            "value.ts",
            "export const text = \"ready\";\nexport let mutable = \"ready\";\n",
        ),
        ("consumer.ts", imports),
    ]);
    let loaded = fixture.load();
    for depth in [0, 31, 32] {
        for (name, summary) in [("text", "\"ready\""), ("mutable", "string")] {
            let answer = loaded.type_of(TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path("consumer.ts"),
                    position: SourcePosition {
                        line: 1,
                        column: imports.find(name).unwrap() + 1,
                    },
                },
                expand: true,
                depth,
            });
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "an established scalar must be described at depth {depth}: {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, summary);
            assert!(!result.description.summary_truncated);
            assert!(result.issues.is_empty());
            let graph = result.description.expansion.unwrap();
            assert_eq!(graph.root, "t0");
            assert_eq!(graph.nodes.len(), 1);
            assert!(!graph.truncated);
            assert_eq!(graph.nodes[0].omitted, None);
            if name == "text" {
                assert_eq!(
                    graph.nodes[0].data,
                    TypeNodeData::Literal {
                        base: PrimitiveName::String,
                        value: "ready".into(),
                    }
                );
            } else {
                assert_eq!(
                    graph.nodes[0].data,
                    TypeNodeData::Primitive {
                        name: PrimitiveName::String
                    }
                );
            }
        }
    }
}

#[test]
fn special_types_are_complete_scalar_graphs_in_both_null_profiles() {
    let cases = [
        ("anyValue", "any", TypeNodeData::Any),
        ("unknownValue", "unknown", TypeNodeData::Unknown),
        ("neverValue", "never", TypeNodeData::Never),
        ("voidValue", "void", TypeNodeData::Void),
        ("undefinedValue", "undefined", TypeNodeData::Undefined),
        ("nullValue", "null", TypeNodeData::Null),
    ];
    let values: String = cases
        .iter()
        .map(|(name, ty, _)| format!("export let {name}: {ty};\n"))
        .collect();
    let imports = format!(
        "import {{ {} }} from './value';\n",
        cases
            .iter()
            .map(|(name, _, _)| *name)
            .collect::<Vec<_>>()
            .join(", ")
    );
    let fixture = project(&[
        ("value.ts", &values),
        ("consumer.ts", &imports),
        (
            "strict.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true},"files":["value.ts","consumer.ts"]}"#,
        ),
        (
            "loose.json",
            r#"{"compilerOptions":{"strict":false,"noLib":true},"files":["value.ts","consumer.ts"]}"#,
        ),
    ]);
    for config in ["strict.json", "loose.json"] {
        let loaded = fixture.load_config(config);
        for depth in [0, 32] {
            for (name, summary, data) in &cases {
                let answer = loaded.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("consumer.ts"),
                        position: SourcePosition {
                            line: 1,
                            column: imports.find(name).unwrap() + 1,
                        },
                    },
                    expand: true,
                    depth,
                });
                assert!(
                    matches!(&answer, Ok(TypeAnswer::Selected(_))),
                    "special {name} must remain an established semantic result: {answer:?}"
                );
                let TypeAnswer::Selected(result) = answer.unwrap() else {
                    unreachable!()
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(&result.description.summary, summary);
                assert!(!result.description.summary_truncated);
                assert!(result.issues.is_empty());
                let graph = result.description.expansion.unwrap();
                assert!(!graph.truncated);
                assert_eq!(graph.nodes.len(), 1);
                assert_eq!(&graph.nodes[0].data, data);
                assert!(graph.nodes[0].omitted.is_none());
            }
        }
    }
}

#[test]
fn normalized_unions_preserve_native_alternatives_and_boolean_display() {
    let source = std::fs::read_to_string("fixtures/types/model-core/cases.ts").unwrap();
    let names = [
        "booleanValue",
        "booleanPair",
        "booleanReduction",
        "anyDominates",
        "unknownDominates",
        "anyUnknown",
        "neverElided",
        "stringReduction",
        "numberReduction",
        "bigintReduction",
        "nestedUnion",
        "stringOrder",
        "numericOrder",
        "mixedOrder",
        "voidUndefined",
    ];
    let imports = format!("import {{ {} }} from './value';\n", names.join(", "));
    let fixture = project(&[
        ("value.ts", &source),
        ("consumer.ts", &imports),
        (
            "strict.json",
            r#"{"compilerOptions":{"strict":true,"noLib":true,"target":"esnext"},"files":["value.ts","consumer.ts"]}"#,
        ),
        (
            "loose.json",
            r#"{"compilerOptions":{"strict":false,"noLib":true,"target":"esnext"},"files":["value.ts","consumer.ts"]}"#,
        ),
    ]);
    for (config, strict) in [("strict.json", true), ("loose.json", false)] {
        let loaded = fixture.load_config(config);
        for name in names {
            let answer = loaded.type_of(TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path("consumer.ts"),
                    position: SourcePosition {
                        line: 1,
                        column: imports.find(name).unwrap() + 1,
                    },
                },
                expand: true,
                depth: 1,
            });
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(_))),
                "normalized union {name} must be established: {answer:?}"
            );
            let TypeAnswer::Selected(result) = answer.unwrap() else {
                unreachable!()
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            let graph = result.description.expansion.unwrap();
            assert!(!graph.truncated);
            let root = &graph.nodes[0];
            let (summary, alternatives): (&str, &[&str]) = match name {
                "booleanValue" | "booleanPair" | "booleanReduction" => ("boolean", &[]),
                "anyDominates" | "anyUnknown" => ("any", &[]),
                "unknownDominates" => ("unknown", &[]),
                "neverElided" | "stringReduction" => ("string", &[]),
                "numberReduction" => ("number", &[]),
                "bigintReduction" => ("bigint", &[]),
                "nestedUnion" => ("string | number", &["string", "number"]),
                "stringOrder" => ("\"a\" | \"b\"", &["\"a\"", "\"b\""]),
                "numericOrder" => ("1 | 2", &["1", "2"]),
                "mixedOrder" if strict => (
                    "string | number | bigint | boolean | null | undefined",
                    &[
                        "undefined",
                        "null",
                        "string",
                        "number",
                        "bigint",
                        "false",
                        "true",
                    ],
                ),
                "mixedOrder" => (
                    "string | number | bigint | boolean",
                    &["string", "number", "bigint", "false", "true"],
                ),
                "voidUndefined" if strict => ("void | undefined", &["undefined", "void"]),
                "voidUndefined" => ("void", &[]),
                _ => unreachable!(),
            };
            assert_eq!(result.description.summary, summary, "{config} {name}");
            if alternatives.is_empty() {
                assert!(
                    !matches!(root.data, TypeNodeData::Union { .. }),
                    "normalized scalar keeps its scalar classification"
                );
                if name.starts_with("boolean") {
                    assert_eq!(
                        root.data,
                        TypeNodeData::Primitive {
                            name: PrimitiveName::Boolean
                        }
                    );
                }
                assert_eq!(graph.nodes.len(), 1);
            } else {
                let TypeNodeData::Union {
                    alternatives: refs,
                    omitted_alternatives,
                } = &root.data
                else {
                    panic!("semantic union remains a union")
                };
                assert_eq!(*omitted_alternatives, None);
                assert_eq!(refs.len(), alternatives.len());
                let displayed: Vec<_> = refs
                    .iter()
                    .map(|reference| {
                        let typescript_parser::types::TypeRef::Ref { r#ref } = reference else {
                            panic!("admitted alternative is present")
                        };
                        graph
                            .nodes
                            .iter()
                            .find(|node| node.id == *r#ref)
                            .unwrap()
                            .summary
                            .as_str()
                    })
                    .collect();
                assert_eq!(displayed, alternatives);
            }
        }
    }
}

#[test]
fn union_depth_zero_retains_the_root_and_exact_omitted_alternative_count() {
    let imports = "import { value } from './value';\n";
    let fixture = project(&[
        ("value.ts", "export let value: 0 | 1 | 2;\n"),
        ("consumer.ts", imports),
    ]);
    let answer = fixture
        .load()
        .type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("consumer.ts"),
                position: SourcePosition {
                    line: 1,
                    column: imports.find("value").unwrap() + 1,
                },
            },
            expand: true,
            depth: 0,
        })
        .unwrap();
    let TypeAnswer::Selected(result) = answer else {
        panic!("union is selected")
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert_eq!(result.description.summary, "0 | 1 | 2");
    let graph = result.description.expansion.unwrap();
    assert!(graph.truncated);
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(
        graph.nodes[0].data,
        TypeNodeData::Union {
            alternatives: vec![],
            omitted_alternatives: Some(3)
        }
    );
    assert_eq!(
        graph.nodes[0].omitted.as_ref().unwrap().reason,
        TypeOmissionReason::Depth
    );
    assert_eq!(
        graph.nodes[0].omitted.as_ref().unwrap().fields,
        vec!["alternatives"]
    );
}

#[test]
fn flat_union_node_boundaries_keep_valid_shallow_input_and_omit_only_the_over_budget_edge() {
    for count in [1998, 1999, 2000] {
        let source =
            std::fs::read_to_string(format!("fixtures/types/model-core/union-{count}.ts")).unwrap();
        let imports = "import { value } from './value';\n";
        let fixture = project(&[("value.ts", &source), ("consumer.ts", imports)]);
        let answer = fixture.load().type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("consumer.ts"),
                position: SourcePosition {
                    line: 1,
                    column: imports.find("value").unwrap() + 1,
                },
            },
            expand: true,
            depth: 1,
        });
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "flat native union remains valid at {count}: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
        assert!(result.description.summary_truncated);
        assert!(serde_json::to_vec(&result.description).unwrap().len() <= 262_144);
        let graph = result.description.expansion.unwrap();
        assert_eq!(graph.nodes.len(), (count + 1).min(2000));
        let TypeNodeData::Union {
            alternatives,
            omitted_alternatives,
        } = &graph.nodes[0].data
        else {
            panic!("root is union")
        };
        assert_eq!(*omitted_alternatives, None);
        assert_eq!(alternatives.len(), count);
        for (index, reference) in alternatives.iter().enumerate() {
            if index == 1999 {
                let typescript_parser::types::TypeRef::Omitted {
                    omitted,
                    kind,
                    summary,
                    summary_truncated,
                } = reference
                else {
                    panic!("over-limit edge is omitted")
                };
                assert_eq!(*omitted, TypeOmissionReason::Nodes);
                assert_eq!(*kind, typescript_parser::types::TypeKind::Literal);
                assert_eq!(summary, "1999");
                assert!(!summary_truncated);
            } else {
                let typescript_parser::types::TypeRef::Ref { r#ref } = reference else {
                    panic!("under/at-limit edge is admitted")
                };
                assert_eq!(r#ref, &format!("t{}", index + 1));
                assert_eq!(
                    graph.nodes[index + 1].data,
                    TypeNodeData::Literal {
                        base: PrimitiveName::Number,
                        value: index.to_string().into()
                    }
                );
            }
        }
    }
}

#[test]
fn zero_depth_retains_object_kind_and_elides_only_nonempty_structural_fields() {
    let imports = "import { current, empty } from './value';\n";
    let fixture = project(&[
        (
            "value.ts",
            "export const current = { label: \"ready\", count: 1 };\nexport const empty = {};\n",
        ),
        ("consumer.ts", imports),
    ]);
    let loaded = fixture.load();
    for (name, truncated) in [("current", true), ("empty", false)] {
        let answer = loaded.type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("consumer.ts"),
                position: SourcePosition {
                    line: 1,
                    column: imports.find(name).unwrap() + 1,
                },
            },
            expand: true,
            depth: 0,
        });
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "depth elision must preserve the established root: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
        let graph = result.description.expansion.unwrap();
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.truncated, truncated);
        let root = &graph.nodes[0];
        let TypeNodeData::Object { members } = &root.data else {
            panic!("elided members retain object kind")
        };
        assert_eq!(members.call_signatures, Some(vec![]));
        assert_eq!(members.construct_signatures, Some(vec![]));
        assert_eq!(members.index_signatures, Some(vec![]));
        if truncated {
            assert_eq!(members.properties, None);
            assert_eq!(
                root.omitted.as_ref().unwrap().reason,
                TypeOmissionReason::Depth
            );
            assert_eq!(root.omitted.as_ref().unwrap().fields, ["properties"]);
        } else {
            assert_eq!(members.properties, Some(vec![]));
            assert_eq!(root.omitted, None);
        }
    }
}

#[test]
fn summary_budget_counts_utf8_and_graph_truncation_includes_text_elision() {
    for payload in [
        "x".repeat(509),
        "x".repeat(510),
        "x".repeat(511),
        format!("{}x", "é".repeat(254)),
        "é".repeat(255),
        format!("{}x", "é".repeat(255)),
    ] {
        let literal = serde_json::to_string(&payload).unwrap();
        let source = format!("export const text = {literal};\n");
        let imports = "import { text } from './value';\n";
        let fixture = project(&[("value.ts", &source), ("consumer.ts", imports)]);
        let loaded = fixture.load();
        let answer = loaded.type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("consumer.ts"),
                position: SourcePosition {
                    line: 1,
                    column: 10,
                },
            },
            expand: true,
            depth: 0,
        });
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "literal must remain established: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        let truncated = literal.len() > 512;
        assert_eq!(
            result.description.summary,
            if truncated { "…" } else { &literal }
        );
        assert_eq!(result.description.summary_truncated, truncated);
        assert!(result.description.summary.len() <= 512);
        let graph = result.description.expansion.unwrap();
        assert_eq!(
            graph.truncated, truncated,
            "actual summary text elision is a display truncation"
        );
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].summary_truncated, truncated);
        assert_eq!(
            graph.nodes[0].data,
            TypeNodeData::Literal {
                base: PrimitiveName::String,
                value: payload.into()
            }
        );
        assert_eq!(graph.nodes[0].omitted, None);
        assert!(result.issues.is_empty());
    }
}

#[test]
fn bounded_deep_summary_does_not_mark_a_later_shared_scalar_as_recursive() {
    let mut deep = "1".to_string();
    for _ in 0..31 {
        deep = format!("{{ a: {deep} }}");
    }
    let source = format!("export const current = {{ deep: {deep}, sibling: 2 }};\n");
    let imports = "import { current } from './value';\n";
    let fixture = project(&[("value.ts", &source), ("consumer.ts", imports)]);
    let answer = fixture.load().type_of(TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("consumer.ts"),
            position: SourcePosition {
                line: 1,
                column: 10,
            },
        },
        expand: false,
        depth: 0,
    });
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(_))),
        "summary depth is display policy: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert!(result.description.summary_truncated);
    assert!(
        result.description.summary.ends_with("sibling: number; }"),
        "a completed deep path must not pollute the following shared scalar: {}",
        result.description.summary
    );
    assert_eq!(result.description.expansion, None);
    assert!(result.issues.is_empty());
}

fn object_description(source: &str) -> typescript_parser::types::TypeDescription {
    let imports = "import { current } from './value';\n";
    let fixture = project(&[("value.ts", source), ("consumer.ts", imports)]);
    let answer = fixture.load().type_of(TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("consumer.ts"),
            position: SourcePosition {
                line: 1,
                column: 10,
            },
        },
        expand: true,
        depth: 3,
    });
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(_))),
        "an established object must produce a bounded description: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert!(result.issues.is_empty());
    result.description
}

#[test]
fn wide_object_uses_a_stable_byte_prefix_and_exact_omitted_count() {
    let count = 2000;
    let rows = (0..count)
        .map(|index| format!("p{index}: 1"))
        .collect::<Vec<_>>()
        .join(",\n");
    let source = format!("export const current = {{\n{rows}\n}};\n");
    let description = object_description(&source);
    assert!(serde_json::to_vec(&description).unwrap().len() <= 262_144);
    let graph = description.expansion.unwrap();
    assert!(graph.truncated);
    assert_eq!(
        graph.nodes.len(),
        2,
        "all property reads share the native number type"
    );
    let root = &graph.nodes[0];
    let TypeNodeData::Object { members } = &root.data else {
        panic!("budget elision retains object kind")
    };
    let shown = members.properties.as_ref().unwrap();
    assert!(!shown.is_empty() && shown.len() < count);
    assert_eq!(members.omitted_properties, Some(count - shown.len()));
    for (index, property) in shown.iter().enumerate() {
        assert_eq!(
            property.key,
            typescript_parser::types::TypeKey::String {
                name: format!("p{index}")
            }
        );
        assert_eq!(
            property.read_type,
            typescript_parser::types::TypeRef::Ref { r#ref: "t1".into() }
        );
    }
    assert_eq!(
        root.omitted.as_ref().unwrap().reason,
        TypeOmissionReason::Bytes
    );
    assert_eq!(root.omitted.as_ref().unwrap().fields, ["properties"]);
    let again = object_description(&source);
    assert_eq!(again.expansion.unwrap(), graph);
}

#[test]
fn compact_type_field_byte_boundary_keeps_exact_keys_under_and_at_limit() {
    let source = |length| format!("export const current = {{ {}: 1 }};\n", "x".repeat(length));
    let baseline_length = 1024;
    let baseline = object_description(&source(baseline_length));
    let base_bytes = serde_json::to_vec(&baseline).unwrap().len();
    // The fixed record overhead includes a four-digit exclusive end column;
    // the boundary fixture has six digits. Key and origin are both exact.
    let at_length = 262_144 - (base_bytes - baseline_length) - 2;
    for (length, expected_bytes) in [(at_length - 1, 262_143), (at_length, 262_144)] {
        let description = object_description(&source(length));
        assert_eq!(
            serde_json::to_vec(&description).unwrap().len(),
            expected_bytes
        );
        let graph = description.expansion.unwrap();
        let TypeNodeData::Object { members } = &graph.nodes[0].data else {
            unreachable!()
        };
        assert_eq!(
            members.properties.as_ref().unwrap()[0].key,
            typescript_parser::types::TypeKey::String {
                name: "x".repeat(length)
            }
        );
        assert_eq!(members.omitted_properties, None);
        assert_eq!(graph.nodes[0].omitted, None);
    }
    let over = object_description(&source(at_length + 1));
    assert!(serde_json::to_vec(&over).unwrap().len() <= 262_144);
    let graph = over.expansion.unwrap();
    let root = &graph.nodes[0];
    let TypeNodeData::Object { members } = &root.data else {
        unreachable!()
    };
    assert_eq!(members.properties, Some(vec![]));
    assert_eq!(members.omitted_properties, Some(1));
    assert_eq!(
        root.omitted.as_ref().unwrap().reason,
        TypeOmissionReason::Bytes
    );
    assert_eq!(root.omitted.as_ref().unwrap().fields, ["properties"]);
}

#[test]
fn final_summary_clause_uses_its_closing_token_instead_of_a_spare_marker() {
    for length in [498, 499, 500] {
        let key = "x".repeat(length);
        let source = format!("export const current = {{ {key}: 1 }};\n");
        let complete = format!("{{ {key}: number; }}");
        assert_eq!(complete.len(), length + 13);
        let description = object_description(&source);
        if complete.len() <= 512 {
            assert_eq!(
                description.summary, complete,
                "a complete 511/512-byte clause fits"
            );
            assert!(!description.summary_truncated);
            assert!(!description.expansion.unwrap().truncated);
        } else {
            assert_eq!(description.summary, "{ … 1 more … }");
            assert!(description.summary_truncated);
            assert!(description.expansion.unwrap().truncated);
        }
    }
}

#[test]
fn breadth_first_ids_share_a_scalar_at_its_shortest_root_distance() {
    let source = "export const current = { deep: { leaf: 1 }, direct: 2 };\n";
    let imports = "import { current } from './value';\n";
    let fixture = project(&[("value.ts", source), ("consumer.ts", imports)]);
    let loaded = fixture.load();
    for depth in [1, 2] {
        let answer = loaded.type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("consumer.ts"),
                position: SourcePosition {
                    line: 1,
                    column: 10,
                },
            },
            expand: true,
            depth,
        });
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        let graph = result.description.expansion.unwrap();
        assert_eq!(
            graph
                .nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            ["t0", "t1", "t2"]
        );
        let TypeNodeData::Object { members } = &graph.nodes[0].data else {
            unreachable!()
        };
        let properties = members.properties.as_ref().unwrap();
        assert_eq!(
            properties[0].read_type,
            typescript_parser::types::TypeRef::Ref { r#ref: "t1".into() }
        );
        assert_eq!(
            properties[1].read_type,
            typescript_parser::types::TypeRef::Ref { r#ref: "t2".into() }
        );
        assert_eq!(
            graph.nodes[2].data,
            TypeNodeData::Primitive {
                name: PrimitiveName::Number
            }
        );
        assert_eq!(graph.nodes[2].omitted, None);
        let TypeNodeData::Object { members } = &graph.nodes[1].data else {
            unreachable!()
        };
        if depth == 1 {
            assert_eq!(members.properties, None);
            assert_eq!(
                graph.nodes[1].omitted.as_ref().unwrap().reason,
                TypeOmissionReason::Depth
            );
            assert!(graph.truncated);
        } else {
            assert_eq!(
                members.properties.as_ref().unwrap()[0].read_type,
                typescript_parser::types::TypeRef::Ref { r#ref: "t2".into() }
            );
            assert_eq!(graph.nodes[1].omitted, None);
            assert!(!graph.truncated);
        }
    }
}

#[test]
fn encountered_lazy_failure_preserves_partial_at_the_node_record_boundary() {
    // Native 7.0.2 admits every union alternative and resolves the late
    // getter-dependent member to string | 0. BFS reaches that member only after
    // reserving the wide union's scalar nodes; bytes remain below their cap.
    for alternatives in [1995, 1996, 1997] {
        let union = (0..alternatives)
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(" | ");
        let source = format!(
            "{FLOW_DEPENDENCY}\nexport interface Budget {{ values: {union}; late: Late; }}\nexport interface Late {{ blocked: typeof dependency.value; }}\n"
        );
        let fixture = project(&[
            ("value.ts", &source),
            ("consumer.ts", "import { Budget } from './value';\n"),
        ]);
        let answer = fixture.load().type_of(TypeRequest {
            query: TypeQuery::At {
                file: fixture.path("consumer.ts"),
                position: SourcePosition {
                    line: 1,
                    column: 10,
                },
            },
            expand: true,
            depth: 2,
        });
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(_))),
            "an encountered failure stays partial at the node cap: {answer:?}"
        );
        let TypeAnswer::Selected(result) = answer.unwrap() else {
            unreachable!()
        };
        assert_eq!(result.outcome, TypeOutcome::Partial);
        assert_eq!(result.issues.len(), 1);
        let issue = &result.issues[0];
        assert_eq!(issue.location, flow_location("value.ts"));
        assert_eq!(issue.reason, SemanticFailureReason::FlowRequired);
        assert!(serde_json::to_vec(&result.description).unwrap().len() < 262_144);
        let graph = result.description.expansion.as_ref().unwrap();
        assert_eq!(graph.nodes.len(), (alternatives + 4).min(2000));
        let TypeNodeData::Union {
            alternatives: shown,
            omitted_alternatives,
        } = &graph.nodes[1].data
        else {
            panic!("the union is discovered before the late named object")
        };
        assert_eq!(shown.len(), alternatives);
        assert_eq!(*omitted_alternatives, None);
        let TypeNodeData::Object { members } = &graph.nodes[2].data else {
            panic!("the late named root remains established")
        };
        assert_eq!(members.omitted_properties, None);
        let child = &members.properties.as_ref().unwrap()[0].read_type;
        if alternatives < 1997 {
            assert_eq!(
                child,
                &typescript_parser::types::TypeRef::Ref {
                    r#ref: format!("t{}", alternatives + 3)
                }
            );
            assert!(
                matches!(&graph.nodes.last().unwrap().data, TypeNodeData::Unsupported { location, message, .. }
                if location == &issue.location && message == &issue.message)
            );
        } else {
            assert!(matches!(child, typescript_parser::types::TypeRef::Omitted {
                omitted: TypeOmissionReason::Nodes,
                kind: typescript_parser::types::TypeKind::Unsupported,
                summary,
                summary_truncated: false,
            } if summary == "unsupported"));
            assert!(
                !graph
                    .nodes
                    .iter()
                    .any(|node| matches!(node.data, TypeNodeData::Unsupported { .. }))
            );
        }
    }
}

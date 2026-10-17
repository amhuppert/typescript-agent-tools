//! Raw symbol reads and independent writes, pinned by native member controls.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{
    AnalysisError, InputErrorKind, SemanticErrorKind, SemanticFailureReason,
};
use typescript_parser::project::Project;
use typescript_parser::types::{
    PrimitiveName, TypeAccessibility, TypeAnswer, TypeExpansion, TypeKey, TypeMembers,
    TypeNodeData, TypeOutcome, TypePropertyKind, TypeQuery, TypeQuestion, TypeRef, TypeRequest,
    TypeResult,
};

const CASES: &str = include_str!("../fixtures/types/native-member-values/cases.ts");
const RECOVERY: &str = include_str!("../fixtures/types/native-member-values/recovery.ts");

fn fixture(strict: bool, exact: bool) -> Fixture {
    let config = format!(
        r#"{{"compilerOptions":{{"strict":true,"strictNullChecks":{strict},"exactOptionalPropertyTypes":{exact},"noLib":true,"types":[]}},"files":["cases.ts","recovery.ts"]}}"#
    );
    Fixture::new(&[
        ("tsconfig.json", &config),
        ("cases.ts", CASES),
        ("recovery.ts", RECOVERY),
    ])
}

fn request(
    fixture: &Fixture,
    file: &str,
    marker: &str,
    skip: usize,
    expand: bool,
    depth: usize,
) -> TypeRequest {
    let source = if file == "cases.ts" { CASES } else { RECOVERY };
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path(file),
        position: position(source, source.find(marker).unwrap() + skip),
    });
    request.expand = expand;
    request.depth = depth;
    request
}

fn selected(loaded: &Project, request: TypeRequest) -> TypeResult {
    let answer = loaded.type_of(request);
    let Ok(TypeAnswer::Selected(result)) = answer else {
        panic!("the native member question must establish its type: {answer:?}")
    };
    assert_eq!(result.outcome, TypeOutcome::Found, "{result:?}");
    assert!(result.issues.is_empty(), "{result:?}");
    result
}

fn node<'a>(graph: &'a TypeExpansion, reference: &TypeRef) -> &'a TypeNodeData {
    let TypeRef::Ref { r#ref } = reference else {
        panic!("the admitted member dependency must be a graph node: {reference:?}")
    };
    &graph
        .nodes
        .iter()
        .find(|node| node.id == *r#ref)
        .unwrap()
        .data
}

fn members(result: &TypeResult) -> (&TypeExpansion, &TypeMembers) {
    let graph = result.description.expansion.as_ref().unwrap();
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    let members = match &root.data {
        TypeNodeData::Object { members } | TypeNodeData::Function { members } => members,
        other => panic!("the established structured type must have members: {other:?}"),
    };
    (graph, members)
}

#[test]
fn merged_property_accessor_writes_use_property_flag_before_accessor_family() {
    let source = include_str!("../fixtures/types/native-member-values/merged-flags.ts");
    let fixture = Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-member-values/merged-flags.json"),
        ),
        ("merged-flags.ts", source),
    ]);
    let project = fixture.load();
    for name in ["mixed", "genericMixed"] {
        let mut request = TypeRequest::new(TypeQuery::At {
            file: fixture.path("merged-flags.ts"),
            position: position(source, source.find(&format!("{name}:")).unwrap()),
        });
        request.expand = true;
        request.depth = 2;
        let result = selected(&project, request);
        let (graph, members) = members(&result);
        let properties = members.properties.as_ref().unwrap();
        assert_eq!(properties.len(), 1, "{name}");
        let property = &properties[0];
        assert_eq!(
            property.key,
            TypeKey::String {
                name: "value".into()
            }
        );
        assert_eq!(
            node(graph, &property.read_type),
            &TypeNodeData::Primitive {
                name: PrimitiveName::String
            },
            "{name}"
        );
        assert!(
            property.write_type.is_none(),
            "{name}: native read and write are the same String identity, so writeType is omitted"
        );
        assert!(!graph.truncated, "{name}");
    }
}

#[test]
fn both_accessor_anchors_ask_raw_read_in_the_native_precedence_order() {
    for strict in [true, false] {
        for exact in [true, false] {
            let fixture = fixture(strict, exact);
            if !strict && exact {
                let error = Project::load(&fixture.path("tsconfig.json"))
                    .map_err(support::input_error)
                    .unwrap_err();
                assert_eq!(error.kind, InputErrorKind::Config);
                assert!(
                    error
                        .to_string()
                        .contains("cannot be specified without option 'strictNullChecks'")
                );
                continue;
            }
            let loaded = fixture.load();
            for (marker, expected) in [
                ("get getterOnly", "string"),
                ("set setterOnly", "number"),
                ("get paired", "string"),
                ("set paired", "string"),
                ("get fromSetter", "boolean"),
                ("set fromSetter", "boolean"),
                ("get ignoredBadSetter", "string"),
                ("set ignoredBadSetter", "string"),
            ] {
                let result = selected(&loaded, request(&fixture, "cases.ts", marker, 4, true, 2));
                assert_eq!(result.target.kind, TypeQuestion::Value);
                assert_eq!(
                    result.description.summary, expected,
                    "{strict}/{exact}/{marker}"
                );
                assert!(!result.description.summary_truncated);
                assert!(!result.description.expansion.as_ref().unwrap().truncated);
            }
        }
    }
}

#[test]
fn optional_property_and_method_raw_reads_keep_strict_option_states() {
    for strict in [true, false] {
        for exact in [true, false] {
            let fixture = fixture(strict, exact);
            if !strict && exact {
                let error = Project::load(&fixture.path("tsconfig.json"))
                    .map_err(support::input_error)
                    .unwrap_err();
                assert_eq!(error.kind, InputErrorKind::Config);
                assert!(
                    error
                        .to_string()
                        .contains("cannot be specified without option 'strictNullChecks'")
                );
                continue;
            }
            let loaded = fixture.load();
            for (marker, expected) in [
                (
                    "optional?:",
                    if strict {
                        "string | undefined"
                    } else {
                        "string"
                    },
                ),
                (
                    "optionalMethod?",
                    if strict {
                        "((value: number) => string) | undefined"
                    } else {
                        "(value: number) => string"
                    },
                ),
            ] {
                let result = selected(&loaded, request(&fixture, "cases.ts", marker, 0, true, 3));
                assert_eq!(result.target.kind, TypeQuestion::Value);
                assert_eq!(
                    result.description.summary, expected,
                    "{strict}/{exact}/{marker}"
                );
                let graph = result.description.expansion.as_ref().unwrap();
                let root = &graph.nodes[0].data;
                if strict {
                    let TypeNodeData::Union { alternatives, .. } = root else {
                        panic!("strict raw optional read retains its nullable identity: {root:?}")
                    };
                    assert!(
                        alternatives
                            .iter()
                            .any(|reference| node(graph, reference) == &TypeNodeData::Undefined)
                    );
                } else {
                    assert!(!matches!(root, TypeNodeData::Union { .. }));
                }
                assert!(!graph.truncated);
            }
        }
    }
}

#[test]
fn ordinary_and_overloaded_methods_reuse_actual_signature_construction() {
    let fixture = fixture(true, false);
    let loaded = fixture.load();
    for (marker, expected, returns) in [
        (
            "method(value:",
            "(value: string) => number",
            vec![PrimitiveName::Number],
        ),
        (
            "overloaded(value:",
            "{ (value: string): string; (value: number): number; }",
            vec![PrimitiveName::String, PrimitiveName::Number],
        ),
    ] {
        let result = selected(&loaded, request(&fixture, "cases.ts", marker, 0, true, 3));
        assert_eq!(result.description.summary, expected);
        let (graph, members) = members(&result);
        let calls = members.call_signatures.as_ref().unwrap();
        assert_eq!(calls.len(), returns.len());
        for (call, expected) in calls.iter().zip(returns) {
            assert_eq!(
                node(graph, &call.return_type),
                &TypeNodeData::Primitive { name: expected }
            );
            assert_eq!(call.parameters.len(), 1);
            assert_eq!(call.parameters[0].name, "value");
            assert!(!call.parameters[0].optional);
            assert!(!call.parameters[0].rest);
            assert!(call.type_parameters.is_empty());
        }
        assert!(!graph.truncated);
    }
}

#[test]
fn generic_member_inventory_maps_reads_writes_and_method_signatures_separately() {
    let fixture = fixture(true, true);
    let loaded = fixture.load();
    let result = selected(
        &loaded,
        request(&fixture, "cases.ts", "generic:", 0, true, 4),
    );
    assert_eq!(result.description.summary, "Generic<string>");
    let (graph, members) = members(&result);
    let properties = members.properties.as_ref().unwrap();
    assert_eq!(
        properties
            .iter()
            .map(|property| &property.key)
            .collect::<Vec<_>>(),
        vec![
            &TypeKey::String {
                name: "method".into()
            },
            &TypeKey::String {
                name: "value".into()
            },
        ]
    );
    let method = &properties[0];
    assert_eq!(method.kind, TypePropertyKind::Method);
    assert!(
        method.write_type.is_none(),
        "same actual read/write identity omits writeType"
    );
    let TypeNodeData::Function {
        members: method_members,
    } = node(graph, &method.read_type)
    else {
        panic!("the instantiated method uses its actual anonymous signature inventory")
    };
    let calls = method_members.call_signatures.as_ref().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].parameters[0].value_type, calls[0].return_type);
    assert_eq!(
        node(graph, &calls[0].return_type),
        &TypeNodeData::Primitive {
            name: PrimitiveName::String
        }
    );
    let accessor = &properties[1];
    assert_eq!(accessor.kind, TypePropertyKind::Accessor);
    assert_eq!(accessor.origins.len(), 2);
    assert_eq!(
        node(graph, &accessor.read_type),
        &TypeNodeData::Primitive {
            name: PrimitiveName::String
        }
    );
    let TypeNodeData::Union { alternatives, .. } =
        node(graph, accessor.write_type.as_ref().unwrap())
    else {
        panic!("the independently instantiated accessor write contains string and number")
    };
    assert_eq!(alternatives.len(), 2);
    assert!(alternatives.iter().any(|reference| node(graph, reference)
        == &TypeNodeData::Primitive {
            name: PrimitiveName::String
        }));
    assert!(alternatives.iter().any(|reference| node(graph, reference)
        == &TypeNodeData::Primitive {
            name: PrimitiveName::Number
        }));
    assert!(!graph.truncated);
}

#[test]
fn class_inventory_keeps_instance_sides_and_actual_accessibility() {
    let fixture = fixture(true, false);
    let loaded = fixture.load();
    let result = selected(
        &loaded,
        request(&fixture, "cases.ts", "declared:", 0, true, 3),
    );
    assert_eq!(result.description.summary, "Declared");
    let (graph, members) = members(&result);
    let properties = members.properties.as_ref().unwrap();
    assert_eq!(properties.len(), 4);
    for (property, (name, kind, accessibility)) in properties.iter().zip([
        (
            "protectedValue",
            TypePropertyKind::Property,
            TypeAccessibility::Protected,
        ),
        (
            "hiddenValue",
            TypePropertyKind::Property,
            TypeAccessibility::Private,
        ),
        (
            "paired",
            TypePropertyKind::Accessor,
            TypeAccessibility::Public,
        ),
        (
            "method",
            TypePropertyKind::Method,
            TypeAccessibility::Public,
        ),
    ]) {
        assert_eq!(property.key, TypeKey::String { name: name.into() });
        assert_eq!(property.kind, kind);
        assert_eq!(property.accessibility, accessibility);
        assert!(!property.optional);
    }
    assert!(!graph.truncated);
}

#[test]
fn nominal_summary_and_depth_zero_do_not_consume_hidden_read_or_write_values() {
    let fixture = fixture(true, false);
    let loaded = fixture.load();
    for expand in [false, true] {
        let result = selected(
            &loaded,
            request(
                &fixture,
                "cases.ts",
                "export declare let plain:",
                "export declare let ".len(),
                expand,
                0,
            ),
        );
        assert_eq!(result.description.summary, "Plain");
        if expand {
            let (graph, members) = members(&result);
            assert_eq!(graph.nodes.len(), 1);
            assert!(members.properties.is_none());
            assert_eq!(members.omitted_properties, None);
            assert!(
                graph.nodes[0]
                    .omitted
                    .as_ref()
                    .unwrap()
                    .fields
                    .iter()
                    .any(|field| field == "properties")
            );
            assert!(graph.truncated);
        }
    }
    let answer = loaded.type_of(request(
        &fixture,
        "cases.ts",
        "export declare let plain:",
        "export declare let ".len(),
        true,
        1,
    ));
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error))
        if error.kind == SemanticErrorKind::InvalidSemantic
        && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
        "admitting the independently writable invalid setter consumes its recovery: {answer:?}"
    );
    let Err(AnalysisError::Semantic(error)) = answer else {
        unreachable!()
    };
    assert_eq!(
        error.position,
        Some(position(CASES, CASES.find("MissingWrite").unwrap()))
    );
}

#[test]
fn genuine_accessor_any_and_unknown_are_distinct_from_consumed_missing_read() {
    for strict in [true, false] {
        let fixture = fixture(strict, false);
        let loaded = fixture.load();
        for (marker, summary, expected) in [
            ("get genuineAny", "any", TypeNodeData::Any),
            ("set genuineAny", "any", TypeNodeData::Any),
            ("get genuineUnknown", "unknown", TypeNodeData::Unknown),
        ] {
            let result = selected(
                &loaded,
                request(&fixture, "recovery.ts", marker, 4, true, 0),
            );
            assert_eq!(result.description.summary, summary);
            assert_eq!(
                result.description.expansion.as_ref().unwrap().nodes[0].data,
                expected
            );
        }
        for marker in ["get badRead", "set badRead"] {
            let answer = loaded.type_of(request(&fixture, "recovery.ts", marker, 4, false, 0));
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error))
                if error.kind == SemanticErrorKind::InvalidSemantic
                && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
                "an accessor's consumed missing read cannot become genuine Any: {answer:?}"
            );
        }
    }
}

#[test]
fn setter_value_parameter_uses_the_complete_native_parameter_choice() {
    let original = include_str!("../fixtures/types/native-member-values/setter-parameters.ts");
    for line_break in ["\r", "\n", "\r\n", "\u{2028}", "\u{2029}"] {
        for bom in ["", "\u{feff}"] {
            let source = format!("{bom}{}", original.replace('\n', line_break));
            let fixture = support::project(&[("setters.ts", &source)]);
            let loaded = fixture.load();
            for (name, summary) in [
                ("none", "any"),
                ("one", "string"),
                ("two", "number"),
                ("twoOrdinary", "boolean"),
                ("three", "Setters"),
            ] {
                let marker = format!("set {name}(");
                let result = selected(
                    &loaded,
                    TypeRequest::new(TypeQuery::At {
                        file: fixture.path("setters.ts"),
                        position: position(&source, source.find(&marker).unwrap() + "set ".len()),
                    }),
                );
                assert_eq!(result.target.kind, TypeQuestion::Value);
                assert_eq!(
                    result.description.summary, summary,
                    "{line_break:?}/{bom:?}/{name}"
                );
            }
        }
    }
}

#[test]
fn conditional_property_reads_and_writes_resolve_the_native_number_type() {
    // The formerly unsupported conditional now resolves to number. Keep the
    // property/accessor and option matrix through the final graph edges.
    const PREFIX: &str = "type Pending = string extends number ? string : number;\n";
    for (declarations, count) in [
        (
            "interface Shape { value: Pending; } declare const selected: Shape;",
            1,
        ),
        (
            "interface Shape { value?: Pending; } declare const selected: Shape;",
            1,
        ),
        ("declare const selected: { value: Pending; };", 1),
        (
            "interface Shape { first: Pending; second: Pending; } declare const selected: Shape;",
            2,
        ),
        (
            "interface Shape { get value(): Pending; set value(input: Pending); } declare const selected: Shape;",
            2,
        ),
    ] {
        for (strict, exact) in [(false, false), (true, false), (true, true)] {
            let input = format!("{PREFIX}{declarations}\n");
            let config = format!(
                r#"{{"compilerOptions":{{"strict":true,"strictNullChecks":{strict},"exactOptionalPropertyTypes":{exact},"noLib":true,"types":[]}},"files":["cases.ts"]}}"#
            );
            let fixture = Fixture::new(&[("tsconfig.json", &config), ("cases.ts", &input)]);
            let project = fixture.load();
            let mut query = TypeRequest::new(TypeQuery::At {
                file: fixture.path("cases.ts"),
                position: position(&input, input.find("selected:").unwrap()),
            });
            query.expand = true;
            query.depth = 2;
            let result = selected(&project, query);
            let (graph, members) = members(&result);
            let properties = members.properties.as_ref().unwrap();
            assert_eq!(
                properties.len(),
                if declarations.contains("get value") {
                    1
                } else {
                    count
                }
            );
            for property in properties {
                let read = node(graph, &property.read_type);
                if strict && declarations.contains("value?") {
                    let TypeNodeData::Union {
                        alternatives,
                        omitted_alternatives,
                    } = read
                    else {
                        panic!("optional read must preserve undefined: {read:?}");
                    };
                    assert_eq!(alternatives.len(), 2);
                    assert!(omitted_alternatives.is_none());
                    assert!(alternatives.iter().any(|reference| matches!(
                        node(graph, reference),
                        TypeNodeData::Undefined
                    )));
                    assert!(alternatives.iter().any(|reference| matches!(
                        node(graph, reference),
                        TypeNodeData::Primitive {
                            name: PrimitiveName::Number
                        }
                    )));
                } else {
                    assert_eq!(
                        read,
                        &TypeNodeData::Primitive {
                            name: PrimitiveName::Number
                        }
                    );
                }
                if let Some(write) = &property.write_type {
                    assert!(strict && exact && declarations.contains("value?"));
                    assert_eq!(
                        node(graph, write),
                        &TypeNodeData::Primitive {
                            name: PrimitiveName::Number
                        }
                    );
                }
            }
            assert!(!graph.truncated);
        }
    }
}

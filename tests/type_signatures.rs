//! Final public signature construction, pinned by the fresh compiler controls.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::references::Location;
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeExpansion, TypeMembers, TypeNodeData, TypeOutcome,
    TypePredicateKind, TypePredicateSubject, TypeQuery, TypeQuestion, TypeRef, TypeRequest,
    TypeResult, TypeThis,
};

const FUNCTIONS: &str = include_str!("../fixtures/types/native-signatures/functions.ts");
const CONSUMER: &str = include_str!("../fixtures/types/native-signatures/consumer.ts");
const CONFIG: &str = include_str!("../fixtures/types/native-signatures/tsconfig.json");
fn fixture(strict: bool) -> Fixture {
    let config = CONFIG.replace("\"strict\":true", &format!("\"strict\":{strict}"));
    Fixture::new(&[
        ("tsconfig.json", &config),
        ("functions.ts", FUNCTIONS),
        ("consumer.ts", CONSUMER),
    ])
}
fn query(fixture: &Fixture, file: &str, marker: &str) -> TypeQuery {
    let source = if file == "functions.ts" {
        FUNCTIONS
    } else {
        CONSUMER
    };
    TypeQuery::At {
        file: fixture.path(file),
        position: position(source, source.find(marker).unwrap()),
    }
}
fn selected(fixture: &Fixture, file: &str, marker: &str, expand: bool, depth: usize) -> TypeResult {
    let mut request = TypeRequest::new(query(fixture, file, marker));
    request.expand = expand;
    request.depth = depth;
    let answer = fixture.load().type_of(request);
    assert!(
        matches!(&answer, Ok(TypeAnswer::Selected(_))),
        "the native signature question must establish {file}:{marker}: {answer:?}"
    );
    let TypeAnswer::Selected(result) = answer.unwrap() else {
        unreachable!()
    };
    result
}
fn node<'a>(graph: &'a TypeExpansion, reference: &TypeRef) -> &'a TypeNodeData {
    let TypeRef::Ref { r#ref } = reference else {
        panic!("admitted child must be a graph node: {reference:?}")
    };
    &graph
        .nodes
        .iter()
        .find(|node| node.id == *r#ref)
        .unwrap()
        .data
}
fn function_members(result: &TypeResult) -> (&TypeExpansion, &TypeMembers) {
    let graph = result.description.expansion.as_ref().unwrap();
    let TypeNodeData::Function { members } = &graph.nodes[0].data else {
        panic!("callable/constructable structure must use the designed function node")
    };
    (graph, members)
}

#[test]
fn own_and_renamed_annotated_functions_preserve_parameters_without_checking_bodies() {
    for strict in [true, false] {
        let fixture = fixture(strict);
        for (file, marker) in [
            ("functions.ts", "annotated("),
            ("consumer.ts", "importedAnnotated"),
        ] {
            let result = selected(&fixture, file, marker, true, 3);
            assert_eq!(result.target.kind, TypeQuestion::Value);
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            assert_eq!(
                result.description.summary,
                format!(
                    "(this: {{ label: string; }}, required: string, optional?: {}, initialized?: boolean) => number",
                    if strict {
                        "number | undefined"
                    } else {
                        "number"
                    }
                )
            );
            let (graph, members) = function_members(&result);
            assert!(!graph.truncated);
            assert!(graph.nodes[0].alias.is_none());
            assert!(members.properties.as_ref().unwrap().is_empty());
            assert!(members.construct_signatures.as_ref().unwrap().is_empty());
            assert!(members.index_signatures.as_ref().unwrap().is_empty());
            let calls = members.call_signatures.as_ref().unwrap();
            assert_eq!(calls.len(), 1);
            let signature = &calls[0];
            assert!(signature.type_parameters.is_empty());
            assert!(signature.predicate.is_none());
            assert_eq!(
                signature
                    .parameters
                    .iter()
                    .map(|parameter| (parameter.name.as_str(), parameter.optional, parameter.rest))
                    .collect::<Vec<_>>(),
                vec![
                    ("required", false, false),
                    ("optional", true, false),
                    ("initialized", true, false)
                ]
            );
            let this = signature.this_parameter.as_ref().unwrap();
            assert_eq!(
                (&this.name, this.optional, this.rest),
                (&"this".to_string(), false, false)
            );
            assert!(matches!(
                node(graph, &this.value_type),
                TypeNodeData::Object { .. }
            ));
            assert_eq!(
                node(graph, &signature.parameters[0].value_type),
                &TypeNodeData::Primitive {
                    name: PrimitiveName::String
                }
            );
            if strict {
                assert!(matches!(
                    node(graph, &signature.parameters[1].value_type),
                    TypeNodeData::Union { .. }
                ));
            } else {
                assert_eq!(
                    node(graph, &signature.parameters[1].value_type),
                    &TypeNodeData::Primitive {
                        name: PrimitiveName::Number
                    }
                );
            }
            assert_eq!(
                node(graph, &signature.parameters[2].value_type),
                &TypeNodeData::Primitive {
                    name: PrimitiveName::Boolean
                }
            );
            assert_eq!(
                node(graph, &signature.return_type),
                &TypeNodeData::Primitive {
                    name: PrimitiveName::Number
                }
            );
            assert_eq!(
                signature.origins,
                vec![Location {
                    file: "functions.ts".into(),
                    start: SourcePosition {
                        line: 1,
                        column: 17
                    },
                    end: SourcePosition {
                        line: 1,
                        column: 26
                    }
                }]
            );
        }
    }
}

#[test]
fn a_bodyless_unannotated_return_is_genuine_any() {
    let fixture = fixture(true);
    for (file, marker) in [
        ("functions.ts", "bodyless("),
        ("consumer.ts", "importedBodyless"),
    ] {
        let result = selected(&fixture, file, marker, true, 3);
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
        assert_eq!(result.description.summary, "(value: string) => any");
        let (graph, members) = function_members(&result);
        assert_eq!(
            node(
                graph,
                &members.call_signatures.as_ref().unwrap()[0].return_type
            ),
            &TypeNodeData::Any
        );
    }
}

#[test]
fn function_type_aliases_and_named_interfaces_share_complete_signature_collections() {
    let fixture = fixture(true);
    for (marker, summary, calls, constructs) in [
        ("ImportedCall", "CallType", 1, 0),
        ("ImportedConstructor", "ConstructorType", 0, 1),
        ("ImportedCallable", "Callable", 1, 1),
    ] {
        for depth in [0, 3] {
            let result = selected(&fixture, "consumer.ts", marker, true, depth);
            assert_eq!(result.target.kind, TypeQuestion::Declared);
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            assert_eq!(result.description.summary, summary);
            let (graph, members) = function_members(&result);
            if depth == 0 {
                assert_eq!(members.call_signatures.is_none(), calls != 0);
                assert_eq!(members.construct_signatures.is_none(), constructs != 0);
                assert_eq!(graph.nodes.len(), 1);
                assert!(graph.truncated);
            } else {
                assert_eq!(members.call_signatures.as_ref().unwrap().len(), calls);
                assert_eq!(
                    members.construct_signatures.as_ref().unwrap().len(),
                    constructs
                );
                assert!(!graph.truncated);
            }
            assert!(members.properties.as_ref().unwrap().is_empty());
            assert!(members.index_signatures.as_ref().unwrap().is_empty());
            assert_eq!(
                graph.nodes[0]
                    .alias
                    .as_ref()
                    .map(|alias| alias.name.as_str()),
                if marker == "ImportedCallable" {
                    None
                } else {
                    Some(summary)
                }
            );
        }
    }
}

#[test]
fn a_consumed_unresolved_parameter_annotation_remains_invalid() {
    let fixture = fixture(true);
    let project = fixture.load();
    let mut request = TypeRequest::new(query(&fixture, "functions.ts", "invalid("));
    request.expand = true;
    let answer = project.type_of(request);
    assert!(
        matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
        "consuming missing parameter annotation must preserve native recovery: {answer:?}"
    );
    let Err(AnalysisError::Semantic(error)) = answer else {
        unreachable!()
    };
    assert_eq!(error.path, Some(fixture.path("functions.ts")));
    assert_eq!(
        error.position,
        Some(SourcePosition {
            line: 4,
            column: 40
        })
    );
}

#[test]
fn default_before_required_uses_strong_arity_and_keeps_display_optionality_separate_from_graph_types()
 {
    for strict in [true, false] {
        let fixture = fixture(strict);
        for (marker, summary, expected_flags) in [
            (
                "defaultsBeforeRequired(",
                format!(
                    "(first: {}, required: string, last?: boolean) => number",
                    if strict {
                        "number | undefined"
                    } else {
                        "number"
                    }
                ),
                vec![false, false, true],
            ),
            (
                "defaultBeforeVoid(",
                format!(
                    "(first: {}, required: void) => number",
                    if strict {
                        "number | undefined"
                    } else {
                        "number"
                    }
                ),
                vec![false, false],
            ),
            (
                "thisBeforeDefaults(",
                format!(
                    "(this: string, first: {}, required: string, last?: boolean) => number",
                    if strict {
                        "number | undefined"
                    } else {
                        "number"
                    }
                ),
                vec![false, false, true],
            ),
            (
                "questionBeforeRequired(",
                format!(
                    "(first?: {}, required: string) => number",
                    if strict {
                        "number | undefined"
                    } else {
                        "number"
                    }
                ),
                vec![true, false],
            ),
            (
                "explicitUndefined(",
                format!(
                    "(first: {}, required: string) => number",
                    if strict {
                        "number | undefined"
                    } else {
                        "number"
                    }
                ),
                vec![false, false],
            ),
            (
                "inferredDefaultsBeforeRequired(",
                format!(
                    "(first: {}, required: string) => number",
                    if strict {
                        "number | undefined"
                    } else {
                        "number"
                    }
                ),
                vec![false, false],
            ),
        ] {
            let result = selected(&fixture, "functions.ts", marker, true, 3);
            let (graph, members) = function_members(&result);
            let signature = &members.call_signatures.as_ref().unwrap()[0];
            assert_eq!(
                signature
                    .parameters
                    .iter()
                    .map(|parameter| parameter.optional)
                    .collect::<Vec<_>>(),
                expected_flags,
                "default values before later required parameters are required under isOptionalParameter"
            );
            assert_eq!(result.description.summary, summary);
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            assert!(!graph.truncated);
            if !matches!(marker, "questionBeforeRequired(" | "explicitUndefined(") || !strict {
                assert_eq!(
                    node(graph, &signature.parameters[0].value_type),
                    &TypeNodeData::Primitive {
                        name: PrimitiveName::Number
                    },
                    "display-added undefined must not replace the getTypeOfSymbol graph parameter type"
                );
            } else {
                assert!(matches!(
                    node(graph, &signature.parameters[0].value_type),
                    TypeNodeData::Union { .. }
                ));
            }
        }
    }
}

#[test]
fn a_literal_alias_parameter_and_return_share_the_canonical_literal_without_a_display_alias() {
    for strict in [true, false] {
        let fixture = fixture(strict);
        let result = selected(&fixture, "functions.ts", "literalAlias(", true, 3);
        assert_eq!(
            result.description.summary,
            "(value: \"ready\") => \"ready\""
        );
        let (graph, members) = function_members(&result);
        let signature = &members.call_signatures.as_ref().unwrap()[0];
        assert_eq!(signature.parameters[0].value_type, signature.return_type);
        assert!(graph.nodes.iter().all(|node| node.alias.is_none()));
        assert_eq!(
            node(graph, &signature.return_type),
            &TypeNodeData::Literal {
                base: PrimitiveName::String,
                value: "ready".into()
            }
        );
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
        assert!(!graph.truncated);
    }
}

#[test]
fn ordered_function_declarations_exclude_only_adjacent_overload_implementations() {
    let fixture = fixture(true);
    for (name, imported, summary, declarations) in [
        (
            "overloaded",
            "importedOverloaded",
            "{ (value: string): string; (value: number): number; }",
            vec![
                ("overloaded(value: string)", PrimitiveName::String),
                ("overloaded(value: number)", PrimitiveName::Number),
            ],
        ),
        (
            "separated",
            "importedSeparated",
            "{ (value: string): string; (value: number): number; }",
            vec![
                ("separated(value: string)", PrimitiveName::String),
                ("separated(value: number)", PrimitiveName::Number),
            ],
        ),
        (
            "commented",
            "importedCommented",
            "(value: string) => string",
            vec![("commented(value: string)", PrimitiveName::String)],
        ),
    ] {
        for (file, marker) in [
            ("functions.ts", format!("{name}(")),
            ("consumer.ts", imported.into()),
        ] {
            let result = selected(&fixture, file, &marker, true, 3);
            assert_eq!(result.description.summary, summary);
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            let (graph, members) = function_members(&result);
            let signatures = members.call_signatures.as_ref().unwrap();
            assert_eq!(signatures.len(), declarations.len());
            for (signature, (marker, primitive)) in signatures.iter().zip(&declarations) {
                let start = FUNCTIONS.find(marker).unwrap();
                assert_eq!(
                    signature.origins,
                    vec![Location {
                        file: "functions.ts".into(),
                        start: position(FUNCTIONS, start),
                        end: position(FUNCTIONS, start + name.len())
                    }]
                );
                assert_eq!(
                    node(graph, &signature.parameters[0].value_type),
                    &TypeNodeData::Primitive { name: *primitive }
                );
                assert_eq!(signature.parameters[0].value_type, signature.return_type);
            }
        }
    }
    let query = TypeQuery::At {
        file: fixture.path("functions.ts"),
        position: position(FUNCTIONS, FUNCTIONS.rfind("overloaded(").unwrap()),
    };
    let mut request = TypeRequest::new(query);
    request.expand = true;
    let TypeAnswer::Selected(result) = fixture.load().type_of(request).unwrap() else {
        panic!("the unrelated namespace function is selected")
    };
    assert_eq!(result.description.summary, "(value: boolean) => boolean");
    let (graph, members) = function_members(&result);
    let calls = members.call_signatures.as_ref().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        node(graph, &calls[0].return_type),
        &TypeNodeData::Primitive {
            name: PrimitiveName::Boolean
        }
    );
    let result = selected(&fixture, "functions.ts", "parentSeparated(", true, 3);
    assert_eq!(
        result.description.summary,
        "{ (value: string): string; (value: number): number; }"
    );
    let (graph, members) = function_members(&result);
    let calls = members.call_signatures.as_ref().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "different namespace body parents retain both canonical function declarations"
    );
    assert_eq!(
        node(graph, &calls[0].return_type),
        &TypeNodeData::Primitive {
            name: PrimitiveName::String
        }
    );
    assert_eq!(
        node(graph, &calls[1].return_type),
        &TypeNodeData::Primitive {
            name: PrimitiveName::Number
        }
    );
}

#[test]
fn explicit_predicates_preserve_subject_kind_asserted_type_and_distinct_return_type() {
    for strict in [true, false] {
        let fixture = fixture(strict);
        for (name, summary, kind, subject, asserted, boolean_return) in [
            (
                "isText",
                "(value: unknown) => value is string",
                TypePredicateKind::Is,
                TypePredicateSubject::ParameterIndex(0),
                true,
                true,
            ),
            (
                "assertText",
                "(value: unknown) => asserts value is string",
                TypePredicateKind::AssertsIs,
                TypePredicateSubject::ParameterIndex(0),
                true,
                false,
            ),
            (
                "assertPresent",
                "(value: unknown) => asserts value",
                TypePredicateKind::Asserts,
                TypePredicateSubject::ParameterIndex(0),
                false,
                false,
            ),
            (
                "isThis",
                "(this: unknown) => this is string",
                TypePredicateKind::Is,
                TypePredicateSubject::This(TypeThis::This),
                true,
                true,
            ),
            (
                "assertThis",
                "(this: unknown) => asserts this is string",
                TypePredicateKind::AssertsIs,
                TypePredicateSubject::This(TypeThis::This),
                true,
                false,
            ),
        ] {
            let result = selected(&fixture, "functions.ts", &format!("{name}("), true, 3);
            assert_eq!(result.description.summary, summary);
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            let (graph, members) = function_members(&result);
            assert!(!graph.truncated);
            let signature = &members.call_signatures.as_ref().unwrap()[0];
            let predicate = signature.predicate.as_ref().unwrap();
            assert_eq!((predicate.kind, predicate.subject), (kind, subject));
            assert_eq!(predicate.value_type.is_some(), asserted);
            if let Some(asserted) = &predicate.value_type {
                assert_eq!(
                    node(graph, asserted),
                    &TypeNodeData::Primitive {
                        name: PrimitiveName::String
                    }
                );
            }
            assert_eq!(
                node(graph, &signature.return_type),
                if boolean_return {
                    &TypeNodeData::Primitive {
                        name: PrimitiveName::Boolean,
                    }
                } else {
                    &TypeNodeData::Void
                }
            );
        }
    }
}

#[test]
fn an_asserted_missing_type_is_invalid_only_when_its_predicate_question_is_demanded() {
    let fixture = fixture(true);
    for (expand, depth) in [(false, 3), (true, 0)] {
        let result = selected(
            &fixture,
            "functions.ts",
            "PredicateContainer",
            expand,
            depth,
        );
        assert_eq!(result.description.summary, "PredicateContainer");
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
    }
    for marker in ["predicateMissing(", "PredicateContainer"] {
        let mut request = TypeRequest::new(query(&fixture, "functions.ts", marker));
        request.expand = true;
        let answer = fixture.load().type_of(request);
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
            "consuming the asserted missing type must preserve native recovery: {answer:?}"
        );
    }
}

#[test]
fn generic_function_parameters_reuse_canonical_read_and_return_identities() {
    for strict in [true, false] {
        let fixture = fixture(strict);
        for (name, imported, summary, decoded, constraint, default) in [
            (
                "generic",
                Some("importedGeneric"),
                "<T extends string = \"ready\">(value: T) => T",
                "T",
                Some(TypeNodeData::Primitive {
                    name: PrimitiveName::String,
                }),
                Some(TypeNodeData::Literal {
                    base: PrimitiveName::String,
                    value: "ready".into(),
                }),
            ),
            (
                "plainGeneric",
                Some("importedPlainGeneric"),
                "<T>(value: T) => T",
                "T",
                None,
                None,
            ),
            (
                "constGeneric",
                Some("importedConstGeneric"),
                "<const T extends string = \"ready\">(value: T) => T",
                "T",
                Some(TypeNodeData::Primitive {
                    name: PrimitiveName::String,
                }),
                Some(TypeNodeData::Literal {
                    base: PrimitiveName::String,
                    value: "ready".into(),
                }),
            ),
            (
                "escapedGeneric",
                Some("importedEscapedGeneric"),
                "<\\u0055 extends number = 1>(value: \\u0055) => \\u0055",
                "U",
                Some(TypeNodeData::Primitive {
                    name: PrimitiveName::Number,
                }),
                Some(TypeNodeData::Literal {
                    base: PrimitiveName::Number,
                    value: "1".into(),
                }),
            ),
            (
                "genericAny",
                None,
                "<T extends unknown = any>(value: T) => T",
                "T",
                Some(TypeNodeData::Unknown),
                Some(TypeNodeData::Any),
            ),
        ] {
            let mut selections = vec![("functions.ts", format!("{name}<"))];
            if let Some(imported) = imported {
                selections.push(("consumer.ts", imported.into()));
            }
            for (file, marker) in selections {
                let result = selected(&fixture, file, &marker, true, 4);
                assert_eq!(result.description.summary, summary);
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert!(result.issues.is_empty());
                let (graph, members) = function_members(&result);
                assert!(!graph.truncated);
                let signature = &members.call_signatures.as_ref().unwrap()[0];
                assert_eq!(signature.type_parameters.len(), 1);
                let parameter = &signature.type_parameters[0];
                assert_eq!(parameter.name, decoded);
                assert!(parameter.variance.is_none());
                assert_eq!(
                    parameter
                        .constraint
                        .as_ref()
                        .map(|reference| node(graph, reference)),
                    constraint.as_ref()
                );
                assert_eq!(
                    parameter
                        .default
                        .as_ref()
                        .map(|reference| node(graph, reference)),
                    default.as_ref()
                );
                assert_eq!(signature.parameters[0].value_type, signature.return_type);
                let TypeNodeData::TypeParameter {
                    name,
                    constraint: read_constraint,
                    default: read_default,
                } = node(graph, &signature.return_type)
                else {
                    panic!("parameter and return must share their established parameter node")
                };
                assert_eq!(name, decoded);
                assert_eq!(read_constraint, &parameter.constraint);
                assert_eq!(read_default, &parameter.default);
            }
        }
    }
}

#[test]
fn generic_function_constructor_and_interface_syntax_share_real_parameter_questions() {
    let fixture = fixture(true);
    for (marker, summary, construct) in [
        ("ImportedGenericCall", "GenericCall", false),
        ("ImportedGenericConstructor", "GenericConstructor", true),
        ("ImportedGenericCallable", "GenericCallable", false),
    ] {
        for depth in [0, 4] {
            let result = selected(&fixture, "consumer.ts", marker, true, depth);
            assert_eq!(result.description.summary, summary);
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert!(result.issues.is_empty());
            let (graph, members) = function_members(&result);
            let signatures = if construct {
                &members.construct_signatures
            } else {
                &members.call_signatures
            };
            if depth == 0 {
                assert!(signatures.is_none());
                assert_eq!(graph.nodes.len(), 1);
                continue;
            }
            assert!(!graph.truncated);
            let signature = &signatures.as_ref().unwrap()[0];
            let parameter = &signature.type_parameters[0];
            assert_eq!(parameter.name, "T");
            assert_eq!(
                node(graph, parameter.constraint.as_ref().unwrap()),
                &TypeNodeData::Primitive {
                    name: PrimitiveName::String
                }
            );
            assert_eq!(
                node(graph, parameter.default.as_ref().unwrap()),
                &TypeNodeData::Literal {
                    base: PrimitiveName::String,
                    value: "ready".into()
                }
            );
            if construct {
                let TypeNodeData::Object { members } = node(graph, &signature.return_type) else {
                    panic!("the constructor return retains its actual object type")
                };
                assert_eq!(
                    members.properties.as_ref().unwrap()[0].read_type,
                    signature.parameters[0].value_type
                );
            } else {
                assert_eq!(signature.return_type, signature.parameters[0].value_type);
            }
            assert!(
                matches!(node(graph, &signature.parameters[0].value_type), TypeNodeData::TypeParameter { name, .. } if name == "T")
            );
        }
    }
}

#[test]
fn generic_overload_parameters_remain_distinct_and_keep_native_declaration_order() {
    let fixture = fixture(true);
    for (file, marker) in [
        ("functions.ts", "genericOverloaded<"),
        ("consumer.ts", "importedGenericOverloaded"),
    ] {
        let result = selected(&fixture, file, marker, true, 4);
        assert_eq!(
            result.description.summary,
            "{ <T extends string>(value: T): T; <U extends number>(value: U): U; }"
        );
        let (graph, members) = function_members(&result);
        let calls = members.call_signatures.as_ref().unwrap();
        assert_eq!(calls.len(), 2);
        assert_ne!(calls[0].return_type, calls[1].return_type);
        for (index, (signature, name, primitive)) in [
            (0, (&calls[0], "T", PrimitiveName::String)),
            (1, (&calls[1], "U", PrimitiveName::Number)),
        ] {
            assert_eq!(signature.type_parameters[0].name, name);
            assert_eq!(signature.parameters[0].value_type, signature.return_type);
            assert_eq!(
                node(
                    graph,
                    signature.type_parameters[0].constraint.as_ref().unwrap()
                ),
                &TypeNodeData::Primitive { name: primitive }
            );
            assert_eq!(signature.origins[0].start.line, 43 + index);
            assert!(
                matches!(node(graph, &signature.return_type), TypeNodeData::TypeParameter { name: actual, .. } if actual == name)
            );
        }
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
        assert!(!graph.truncated);
    }
    let result = selected(&fixture, "functions.ts", "sameGenericName<", true, 4);
    assert_eq!(
        result.description.summary,
        "{ <T extends string>(value: T): T; <T extends number>(value: T): T; }"
    );
    let (graph, members) = function_members(&result);
    let calls = members.call_signatures.as_ref().unwrap();
    assert_eq!(calls.len(), 2);
    assert_ne!(
        calls[0].return_type, calls[1].return_type,
        "equal parameter spelling must not intern different canonical symbols together"
    );
    for (signature, constraint) in [
        (&calls[0], PrimitiveName::String),
        (&calls[1], PrimitiveName::Number),
    ] {
        assert_eq!(signature.type_parameters[0].name, "T");
        assert_eq!(signature.parameters[0].value_type, signature.return_type);
        assert_eq!(
            node(
                graph,
                signature.type_parameters[0].constraint.as_ref().unwrap()
            ),
            &TypeNodeData::Primitive { name: constraint }
        );
    }
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert!(result.issues.is_empty());
    assert!(!graph.truncated);
}

#[test]
fn declared_generic_modifier_variants_preserve_native_display_order_and_descriptor_variance() {
    let fixture = fixture(true);
    for (name, summary, variance) in [
        ("varianceIn", "<in T>(value: T) => T", "in"),
        ("varianceOut", "<out T>(value: T) => T", "out"),
        ("varianceBoth", "<in out T>(value: T) => T", "in out"),
        ("constVariance", "<const in out T>(value: T) => T", "in out"),
    ] {
        let result = selected(&fixture, "functions.ts", &format!("{name}<"), true, 3);
        assert_eq!(result.description.summary, summary);
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
        let (graph, members) = function_members(&result);
        let signature = &members.call_signatures.as_ref().unwrap()[0];
        let parameter = &signature.type_parameters[0];
        assert_eq!(parameter.name, "T");
        assert_eq!(parameter.variance.as_deref(), Some(variance));
        assert!(parameter.constraint.is_none());
        assert!(parameter.default.is_none());
        assert_eq!(signature.parameters[0].value_type, signature.return_type);
        assert!(
            matches!(node(graph, &signature.return_type), TypeNodeData::TypeParameter { name, constraint: None, default: None } if name == "T")
        );
        assert!(!graph.truncated);
    }
}

#[test]
fn generic_parameter_identity_does_not_force_invalid_constraint_or_default_until_requested() {
    let fixture = fixture(true);
    for (expand, depth) in [(false, 4), (true, 0)] {
        let result = selected(
            &fixture,
            "consumer.ts",
            "ImportedGenericMissing",
            expand,
            depth,
        );
        assert_eq!(result.description.summary, "GenericMissing");
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert!(result.issues.is_empty());
    }
    for (file, marker, expand) in [
        ("consumer.ts", "ImportedGenericMissing", true),
        ("functions.ts", "genericInvalid<", false),
    ] {
        let mut request = TypeRequest::new(query(&fixture, file, marker));
        request.expand = expand;
        let answer = fixture.load().type_of(request);
        assert!(
            matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::UnresolvedSymbol.into()),
            "only consumed constraint/default recovery may invalidate the requested type: {answer:?}"
        );
        let Err(AnalysisError::Semantic(error)) = answer else {
            unreachable!()
        };
        assert_eq!(error.path, Some(fixture.path("functions.ts")));
        let source = FUNCTIONS
            .find(if file == "consumer.ts" {
                "GenericMissing"
            } else {
                "genericInvalid"
            })
            .unwrap();
        let missing = source + FUNCTIONS[source..].find("MissingConstraint").unwrap();
        assert_eq!(error.position, Some(position(FUNCTIONS, missing)));
    }
}

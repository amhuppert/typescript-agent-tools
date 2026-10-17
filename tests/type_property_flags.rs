//! Native declaration flags and prototype binding order reach final queries.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{
    AnalysisError, InputErrorKind, SemanticErrorKind, SemanticFailureReason,
};
use typescript_parser::project::Project;
use typescript_parser::types::{
    TypeAnswer, TypeKey, TypeNodeData, TypeOutcome, TypePropertyKind, TypeQuery, TypeQuestion,
    TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/native-property-flags/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-property-flags/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-property-flags/loose.json"),
        ),
        (
            "exact.json",
            include_str!("../fixtures/types/native-property-flags/exact.json"),
        ),
    ])
}

/// A nominal graph label: the compiler spelling plus ` (file:line:column)` of
/// the class's first declaration name, as the Q4 graph contract requires.
fn qualified(name: &str) -> String {
    let at = SOURCE
        .find(&format!("class {name} "))
        .map(|at| at + "class ".len())
        .or_else(|| SOURCE.find(&format!("*/{name} ")).map(|at| at + "*/".len()))
        .unwrap();
    let at = position(SOURCE, at);
    format!("{name} (cases.ts:{}:{})", at.line, at.column)
}

fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let marker = format!("/*#{marker}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len()),
    });
    request.expand = expand;
    request
}

#[test]
fn class_prototype_reads_and_writes_keep_the_declared_class_type() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [
            ("prototypeRead", "Plain"),
            ("prototypeWrite", "Plain"),
            ("genericPrototypeWrite", "Generic<any>"),
            ("annotatedPrototypeRead", "AnnotatedPrototype"),
            ("getterStringRead", "string"),
            ("accessorStringRead", "string"),
            ("accessorStringWrite", "string"),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{config}/{marker}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{marker}");
                assert_eq!(
                    result.target.kind,
                    TypeQuestion::Expression,
                    "{config}/{marker}"
                );
                assert_eq!(result.description.summary, summary, "{config}/{marker}");
            }
        }
    }
}

#[test]
fn merged_readonly_flags_preserve_value_declaration_order_and_getter_precedence() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, summary) in [
            ("propertyFirstWrite", None),
            ("setterFirstWrite", Some("boolean")),
            ("withGetterWrite", Some("string")),
            ("getterOnlyWrite", None),
            ("mutableWrite", Some("boolean")),
            ("genericReadonlyWrite", None),
            ("readonlyPrototypeWrite", None),
            ("getterPrototypeWrite", None),
            ("accessorPrototypeWrite", Some("AccessorPrototype")),
        ] {
            for expand in [false, true] {
                let answer = project.type_of(request(&fixture, marker, expand));
                if let Some(summary) = summary {
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!("{config}/{marker}: {answer:?}");
                    };
                    assert_eq!(result.outcome, TypeOutcome::Found, "{config}/{marker}");
                    assert_eq!(result.description.summary, summary, "{config}/{marker}");
                } else {
                    assert!(
                        matches!(&answer, Err(AnalysisError::Semantic(error))
                        if error.kind == SemanticErrorKind::InvalidSemantic && error.reason == SemanticFailureReason::ErrorType.into()),
                        "{config}/{marker}: {answer:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn class_name_anchors_select_declared_types_for_each_native_class_form() {
    let fixture = fixture();
    let project = fixture.load();
    for (marker, summary) in [
        ("plainClass", "Plain"),
        ("genericClass", "Generic<T>"),
        ("derivedClass", "Derived"),
        ("expressionClass", "Inside"),
        ("genericExpressionClass", "InsideGeneric<T>"),
        ("defaultClass", "NamedDefault"),
    ] {
        for expand in [false, true] {
            let answer = project.type_of(request(&fixture, marker, expand));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{marker}: {answer:?}");
            };
            assert_eq!(result.target.kind, TypeQuestion::Declared, "{marker}");
            assert_eq!(result.outcome, TypeOutcome::Found, "{marker}");
            assert_eq!(result.description.summary, summary, "{marker}");
        }
    }
}

#[test]
fn expanded_prototypes_keep_actual_declaration_flags_and_origins() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json", "exact.json"] {
        let project = fixture.load_config(config);
        for (marker, class_name, value_type, readonly, kind, origin_count) in [
            (
                "plainStatic",
                "Plain",
                "Plain",
                false,
                TypePropertyKind::Property,
                0,
            ),
            (
                "readonlyStatic",
                "ReadonlyPrototype",
                "ReadonlyPrototype",
                true,
                TypePropertyKind::Property,
                1,
            ),
            (
                "getterStatic",
                "GetterPrototype",
                "GetterPrototype",
                true,
                TypePropertyKind::Accessor,
                1,
            ),
            (
                "accessorStatic",
                "AccessorPrototype",
                "AccessorPrototype",
                false,
                TypePropertyKind::Accessor,
                2,
            ),
            (
                "annotatedStatic",
                "AnnotatedPrototype",
                "AnnotatedPrototype",
                false,
                TypePropertyKind::Property,
                1,
            ),
            (
                "getterStringStatic",
                "GetterStringPrototype",
                "string",
                true,
                TypePropertyKind::Accessor,
                1,
            ),
            (
                "accessorStringStatic",
                "AccessorStringPrototype",
                "string",
                false,
                TypePropertyKind::Accessor,
                2,
            ),
        ] {
            let answer = project.type_of(request(&fixture, marker, true));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{config}/{marker}: {answer:?}");
            };
            assert_eq!(
                result.outcome,
                TypeOutcome::Found,
                "{config}/{marker}: {result:?}"
            );
            assert_eq!(result.description.summary, format!("typeof {class_name}"));
            assert!(result.issues.is_empty(), "{config}/{marker}");
            let graph = result.description.expansion.as_ref().unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            let members = match &root.data {
                TypeNodeData::Object { members } | TypeNodeData::Function { members } => members,
                other => panic!("{config}/{marker}: {other:?}"),
            };
            let prototype = members.properties.as_ref().unwrap().iter().find(|property|
                matches!(&property.key, TypeKey::String {name} if name == "prototype")).unwrap();
            assert_eq!(prototype.readonly, readonly, "{config}/{marker}");
            assert_eq!(prototype.kind, kind, "{config}/{marker}");
            assert_eq!(prototype.origins.len(), origin_count, "{config}/{marker}");
            assert!(
                prototype.write_type.is_none(),
                "equal native read/write types share one graph reference"
            );
            let TypeRef::Ref { r#ref } = &prototype.read_type else {
                panic!("{config}/{marker}: {:?}", prototype.read_type);
            };
            let read = graph.nodes.iter().find(|node| node.id == *r#ref).unwrap();
            let label = if value_type == class_name {
                qualified(value_type)
            } else {
                value_type.to_owned()
            };
            assert_eq!(read.summary, label, "{config}/{marker}");
        }
    }
}

#[test]
fn class_prototype_binding_precedes_later_namespace_exports() {
    for (source, accepted) in [
        (
            include_str!("../fixtures/types/native-property-flags/forward.ts"),
            true,
        ),
        (
            include_str!("../fixtures/types/native-property-flags/reverse.ts"),
            false,
        ),
        (
            include_str!("../fixtures/types/native-property-flags/forward-value.ts"),
            false,
        ),
        (
            include_str!("../fixtures/types/native-property-flags/reverse-value.ts"),
            false,
        ),
        (
            include_str!("../fixtures/types/native-property-flags/forward-alias.ts"),
            true,
        ),
        (
            include_str!("../fixtures/types/native-property-flags/reverse-alias.ts"),
            false,
        ),
        (
            include_str!("../fixtures/types/native-property-flags/forward-type-alias.ts"),
            true,
        ),
        (
            include_str!("../fixtures/types/native-property-flags/reverse-type-alias.ts"),
            false,
        ),
    ] {
        let fixture = Fixture::new(&[
            ("cases.ts", source),
            (
                "tsconfig.json",
                include_str!("../fixtures/types/native-property-flags/tsconfig.json"),
            ),
        ]);
        let result = Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error);
        if accepted {
            assert!(result.is_ok(), "{source}: {result:?}");
        } else {
            assert!(
                matches!(result, Err(error) if error.kind == InputErrorKind::Binding),
                "{source}"
            );
        }
    }
}

//! Canonical named members use the retained binder's actual own associations.
mod support;

use support::{Fixture, position, project};
use typescript_parser::project::Project;
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeKey, TypeNodeData, TypeOutcome, TypeQuery, TypeQuestion,
    TypeRef, TypeRequest,
};

fn value_at(loaded: &Project, fixture: &Fixture, source: &str, marker: &str, summary: &str) {
    let offset = source.find(marker).unwrap() + marker.len();
    let answer = loaded.type_of(TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("members.ts"),
            position: position(source, offset),
        },
        expand: false,
        depth: 0,
    });
    let Ok(TypeAnswer::Selected(result)) = &answer else {
        panic!("canonical member declaration has a native value question: {answer:?}");
    };
    assert_eq!(result.outcome, TypeOutcome::Found, "{answer:?}");
    assert_eq!(result.target.kind, TypeQuestion::Value);
    assert_eq!(result.target.location.start, position(source, offset));
    assert_eq!(result.description.summary, summary);
    assert!(!result.description.summary_truncated);
    assert!(result.issues.is_empty());
}

#[test]
fn merged_property_declarations_consume_the_first_actual_value_declaration() {
    for line_break in ["\r", "\n", "\r\n", "\u{2028}", "\u{2029}"] {
        for (first, second) in [("number", "string"), ("string", "number")] {
            let source = format!(
                "\u{feff}/* astral 😀 */{line_break}interface Bag {{ /*#first*/value: {first}; }}{line_break}interface Bag {{ /*#second*/value: {second}; }}{line_break}"
            );
            let fixture = project(&[("members.ts", &source)]);
            let loaded = fixture.load();
            for marker in ["/*#first*/", "/*#second*/"] {
                value_at(&loaded, &fixture, &source, marker, first);
            }
        }
    }
}

#[test]
fn literal_and_escaped_member_keys_retain_their_actual_value_roles() {
    let source = r#"interface Keys {
  /*#empty*/"": string;
  /*#dotted*/"a.b": number;
  /*#numeric*/0x10: boolean;
  /*#escaped*/\u0065scaped: number;
}"#;
    let fixture = project(&[("members.ts", source)]);
    let loaded = fixture.load();
    for (marker, summary) in [
        ("/*#empty*/", "string"),
        ("/*#dotted*/", "number"),
        ("/*#numeric*/", "boolean"),
        ("/*#escaped*/", "number"),
    ] {
        value_at(&loaded, &fixture, source, marker, summary);
    }
}

#[test]
fn generic_merged_inventory_uses_canonical_order_types_and_complete_own_origins() {
    let source = include_str!("../fixtures/types/native-member-tables/inventory.ts");
    let fixture = project(&[("members.ts", source)]);
    let loaded = fixture.load();
    let offset = source.find("/*#inventory*/").unwrap() + "/*#inventory*/".len();
    let answer = loaded.type_of(TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("members.ts"),
            position: position(source, offset),
        },
        expand: true,
        depth: 3,
    });
    let Ok(TypeAnswer::Selected(result)) = &answer else {
        panic!("canonical merged inventory: {answer:?}");
    };
    assert_eq!(result.outcome, TypeOutcome::Found, "{answer:?}");
    assert_eq!(result.target.kind, TypeQuestion::Type);
    assert_eq!(result.description.summary, "NativeInventory<number>");
    assert!(result.issues.is_empty(), "{answer:?}");
    let graph = result.description.expansion.as_ref().unwrap();
    assert!(!graph.truncated);
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    let TypeNodeData::Object { members, .. } = &root.data else {
        panic!("native inventory shape: {root:?}");
    };
    let properties = members.properties.as_ref().unwrap();
    assert_eq!(
        properties
            .iter()
            .map(|property| &property.key)
            .collect::<Vec<_>>(),
        ["T", "value", "", "a.b", "16", "escaped", "later"]
            .iter()
            .map(|name| TypeKey::String {
                name: (*name).into()
            })
            .collect::<Vec<_>>()
            .iter()
            .collect::<Vec<_>>()
    );
    for (property, (expected, origins)) in properties.iter().zip([
        (PrimitiveName::String, 3),
        (PrimitiveName::Number, 2),
        (PrimitiveName::String, 1),
        (PrimitiveName::Number, 1),
        (PrimitiveName::Boolean, 1),
        (PrimitiveName::Number, 1),
        (PrimitiveName::Boolean, 1),
    ]) {
        let TypeRef::Ref { r#ref: id } = &property.read_type else {
            panic!("admitted member value: {property:?}");
        };
        let node = graph.nodes.iter().find(|node| node.id == *id).unwrap();
        assert_eq!(node.data, TypeNodeData::Primitive { name: expected });
        assert_eq!(
            property.origins.len(),
            origins,
            "complete canonical own declarations: {property:?}"
        );
    }
}

#[test]
fn a_shared_parameter_property_symbol_keeps_declared_and_value_questions_separate() {
    let source = include_str!("../fixtures/types/native-member-tables/inventory.ts");
    let fixture = project(&[("members.ts", source)]);
    let loaded = fixture.load();
    value_at(&loaded, &fixture, source, "/*#propertyT*/", "string");
    let offset = source.find("value: T").unwrap() + "value: ".len();
    let answer = loaded.type_of(TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("members.ts"),
            position: position(source, offset),
        },
        expand: false,
        depth: 0,
    });
    let Ok(TypeAnswer::Selected(result)) = &answer else {
        panic!("shared declared parameter: {answer:?}");
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert_eq!(result.target.kind, TypeQuestion::Type);
    assert_eq!(result.description.summary, "T");
    assert!(result.issues.is_empty());
}

#[test]
fn computed_member_name_operands_keep_their_actual_expression_question() {
    let source = "interface Computed { [\"computed\"]: number; }";
    let fixture = project(&[("members.ts", source)]);
    let loaded = fixture.load();
    let offset = source.find("[\"computed\"]").unwrap() + 1;
    let answer = loaded.type_of(TypeRequest {
        query: TypeQuery::At {
            file: fixture.path("members.ts"),
            position: position(source, offset),
        },
        expand: false,
        depth: 0,
    });
    let Ok(TypeAnswer::Selected(result)) = &answer else {
        panic!("computed name expression: {answer:?}");
    };
    assert_eq!(result.outcome, TypeOutcome::Found);
    assert_eq!(result.target.kind, TypeQuestion::Expression);
    assert_eq!(result.target.location.start, position(source, offset));
    assert_eq!(result.description.summary, "\"computed\"");
    assert!(result.issues.is_empty());
}

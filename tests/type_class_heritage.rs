//! Final graphs and calls consume native instance/static/base constructor factories.
mod support;

use support::{Fixture, position};
use typescript_parser::project::Project;
use typescript_parser::types::{
    TypeAnswer, TypeExpansion, TypeKey, TypeMembers, TypeNode, TypeNodeData, TypeOutcome,
    TypeQuery, TypeRef, TypeRequest, TypeResult,
};

const SOURCE: &str = include_str!("../fixtures/types/native-class-heritage/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-class-heritage/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-class-heritage/loose.json"),
        ),
        ("cases.ts", SOURCE),
    ])
}

fn selected(project: &Project, fixture: &Fixture, name: &str) -> TypeResult {
    let offset = SOURCE
        .find(&format!("{name}:"))
        .or_else(|| SOURCE.find(&format!("{name} =")))
        .unwrap();
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, offset),
    });
    request.expand = true;
    request.depth = 6;
    let answer = project.type_of(request);
    let Ok(TypeAnswer::Selected(result)) = answer else {
        panic!("{name}: {answer:?}")
    };
    assert_eq!(result.outcome, TypeOutcome::Found, "{name}: {result:?}");
    assert!(result.issues.is_empty(), "{name}");
    assert!(
        !result.description.expansion.as_ref().unwrap().truncated,
        "{name}"
    );
    result
}

fn root(result: &TypeResult) -> (&TypeExpansion, &TypeMembers) {
    let graph = result.description.expansion.as_ref().unwrap();
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .unwrap();
    let members = match &root.data {
        TypeNodeData::Object { members } | TypeNodeData::Function { members } => members,
        other => panic!("expected native class structure: {other:?}"),
    };
    (graph, members)
}

fn node<'a>(graph: &'a TypeExpansion, reference: &TypeRef) -> &'a TypeNode {
    let TypeRef::Ref { r#ref } = reference else {
        panic!("completed dependency: {reference:?}")
    };
    graph.nodes.iter().find(|node| node.id == *r#ref).unwrap()
}

fn properties(result: &TypeResult) -> Vec<(String, String)> {
    let (graph, members) = root(result);
    members
        .properties
        .as_ref()
        .unwrap()
        .iter()
        .map(|property| {
            let TypeKey::String { name } = &property.key else {
                panic!("ordinary class key")
            };
            (
                name.clone(),
                node(graph, &property.read_type).summary.clone(),
            )
        })
        .collect()
}

// Q4 graph labels retain the native spelling and qualify the actual class's
// first declaration. Top-level TypeDescription summaries remain unqualified.
// The paired native harness verifies each returned/prototype type's symbol
// association, including generic instances and synthesized constructors.
fn class_graph_label(summary: &str) -> String {
    let name = summary.split('<').next().unwrap();
    let declaration = format!("class {name}");
    let at = SOURCE.find(&declaration).unwrap() + "class ".len();
    let start = position(SOURCE, at);
    format!("{summary} (cases.ts:{}:{})", start.line, start.column)
}

#[test]
fn instance_bases_substitute_generics_and_merge_interfaces_in_native_order() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(profile);
        for (name, summary, expected) in [
            (
                "derivedInstance",
                "Derived",
                vec![
                    ("own", "boolean"),
                    ("base", "string"),
                    ("guarded", "boolean"),
                    ("hidden", "number"),
                ],
            ),
            (
                "explicitInstance",
                "Explicit",
                vec![
                    ("own", "string"),
                    ("base", "number"),
                    ("guarded", "boolean"),
                    ("hidden", "number"),
                ],
            ),
            (
                "mergedInstance",
                "Merged",
                vec![
                    ("own", "boolean"),
                    ("interfaceOwn", "number"),
                    ("base", "number"),
                    ("guarded", "boolean"),
                    ("hidden", "number"),
                    ("interfaceBase", "string"),
                ],
            ),
            (
                "genericInstance",
                "GenericDerived<string>",
                vec![
                    ("own", "string"),
                    ("base", "string"),
                    ("guarded", "boolean"),
                    ("hidden", "number"),
                ],
            ),
            (
                "likeInstance",
                "LikeDerived",
                vec![("own", "boolean"), ("inherited", "number")],
            ),
        ] {
            let result = selected(&project, &fixture, name);
            assert_eq!(result.description.summary, summary, "{profile}/{name}");
            assert_eq!(
                properties(&result),
                expected
                    .into_iter()
                    .map(|(name, ty)| (name.into(), ty.into()))
                    .collect::<Vec<_>>(),
                "{profile}/{name}"
            );
        }
    }
}

#[test]
fn static_exports_inherit_separately_and_prototypes_keep_native_class_types() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(profile);
        for (name, class, expected) in [
            (
                "derivedStatic",
                "Derived",
                vec![
                    ("replaced", "\"derived\""),
                    ("inheritedStatic", "string"),
                    ("prototype", "Derived"),
                ],
            ),
            (
                "explicitStatic",
                "Explicit",
                vec![
                    ("inheritedStatic", "string"),
                    ("replaced", "string"),
                    ("prototype", "Explicit"),
                ],
            ),
            (
                "likeStatic",
                "LikeDerived",
                vec![("shared", "string"), ("prototype", "LikeDerived")],
            ),
            (
                "genericStatic",
                "GenericDerived",
                vec![
                    ("inheritedStatic", "string"),
                    ("replaced", "string"),
                    ("prototype", "GenericDerived<any>"),
                ],
            ),
        ] {
            let result = selected(&project, &fixture, name);
            assert_eq!(result.description.summary, format!("typeof {class}"));
            assert_eq!(
                properties(&result),
                expected
                    .into_iter()
                    .map(|(name, ty)| (
                        name.into(),
                        if name == "prototype" {
                            class_graph_label(ty)
                        } else {
                            ty.into()
                        }
                    ))
                    .collect::<Vec<_>>(),
                "{profile}/{name}"
            );
            let (_, members) = root(&result);
            let prototype = members.properties.as_ref().unwrap().last().unwrap();
            assert!(
                prototype.origins.is_empty(),
                "the native prototype has no declarations"
            );
            assert!(!prototype.readonly, "native Property|Prototype is writable");
        }
    }
}

#[test]
fn explicit_and_default_constructor_graphs_preserve_parameters_and_class_returns() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(profile);
        for (name, returned, parameters, locals) in [
            ("derivedStatic", "Derived", vec!["string"], vec![]),
            (
                "explicitStatic",
                "Explicit",
                vec![
                    "boolean",
                    if profile == "tsconfig.json" {
                        "number | undefined"
                    } else {
                        "number"
                    },
                ],
                vec![],
            ),
            ("abstractStatic", "Abstract", vec!["boolean"], vec![]),
            ("concreteStatic", "Concrete", vec!["boolean"], vec![]),
            ("likeStatic", "LikeDerived", vec!["number"], vec![]),
            ("genericStatic", "GenericDerived<U>", vec!["U"], vec!["U"]),
        ] {
            let result = selected(&project, &fixture, name);
            let (graph, members) = root(&result);
            let constructors = members.construct_signatures.as_ref().unwrap();
            assert_eq!(constructors.len(), 1, "{name}");
            let signature = &constructors[0];
            assert_eq!(
                signature
                    .type_parameters
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect::<Vec<_>>(),
                locals,
                "{name}"
            );
            assert_eq!(
                signature
                    .parameters
                    .iter()
                    .map(|parameter| node(graph, &parameter.value_type).summary.as_str())
                    .collect::<Vec<_>>(),
                parameters,
                "{name}"
            );
            assert_eq!(
                node(graph, &signature.return_type).summary,
                class_graph_label(returned),
                "{name}"
            );
        }
    }
}

#[test]
fn absent_null_and_any_class_bases_keep_distinct_indexes_and_default_signatures() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(profile);
        for (name, class, own) in [
            ("plain", "Plain", "number"),
            ("null", "NullBase", "string"),
            ("any", "AnyDerived", "boolean"),
        ] {
            for side in ["Instance", "Static"] {
                let result = selected(&project, &fixture, &format!("{name}{side}"));
                let (graph, members) = root(&result);
                let indexes = members.index_signatures.as_ref().unwrap();
                assert_eq!(
                    indexes.len(),
                    usize::from(name == "any"),
                    "{profile}/{name}{side}"
                );
                if name == "any" {
                    assert_eq!(node(graph, &indexes[0].key_type).summary, "string");
                    assert_eq!(node(graph, &indexes[0].value_type).summary, "any");
                }
                if side == "Instance" {
                    assert_eq!(properties(&result), vec![("own".into(), own.into())]);
                } else {
                    let constructors = members.construct_signatures.as_ref().unwrap();
                    assert_eq!(constructors.len(), 1);
                    assert!(constructors[0].parameters.is_empty());
                    assert_eq!(
                        node(graph, &constructors[0].return_type).summary,
                        class_graph_label(class)
                    );
                }
            }
        }
    }
}

#[test]
fn abstract_default_constructors_change_final_overload_selection() {
    let fixture = fixture();
    for profile in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(profile);
        for expected in ["abstract", "concrete"] {
            let result = selected(&project, &fixture, &format!("{expected}Result"));
            assert_eq!(result.description.summary, format!("\"{expected}\""));
        }
    }
}

//! Anonymous type node identity and alias display are separate native facts.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-anonymous-types/cases.ts");
const WRITTEN_NAMES: &str =
    include_str!("../fixtures/types/native-anonymous-types/written-names.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-anonymous-types/tsconfig.json"),
        ),
        (
            "loose.json",
            include_str!("../fixtures/types/native-anonymous-types/loose.json"),
        ),
        ("cases.ts", SOURCE),
        ("written-names.ts", WRITTEN_NAMES),
    ])
}

fn request(fixture: &Fixture, marker: &str, expand: bool, depth: usize) -> TypeRequest {
    let token = format!("/*#{marker}*/ ");
    let offset = SOURCE.find(&token).unwrap() + token.len();
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, offset),
    });
    request.expand = expand;
    request.depth = depth;
    request
}

#[test]
fn alias_summaries_keep_written_spelling_while_graph_metadata_keeps_the_decoded_name() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, name, expected) in [
            ("functionAlias", "Fn", r"\u0046n"),
            ("constructorAlias", "Ctor", r"\u0043tor"),
            ("literalAlias", "Literal", r"\u004citeral"),
        ] {
            for (expand, depth) in [(false, 0), (true, 0), (true, 2)] {
                let answer = project.type_of(request(&fixture, marker, expand, depth));
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("native alias question must succeed ({marker}): {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(
                    result.description.summary, expected,
                    "{config}/{marker}/{depth}"
                );
                assert!(result.issues.is_empty());
                if let Some(graph) = result.description.expansion {
                    let root = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == graph.root)
                        .unwrap();
                    let alias = root
                        .alias
                        .as_ref()
                        .expect("actual alias provenance is retained");
                    assert_eq!(alias.name, name);
                    assert_eq!(alias.origins.len(), 1);
                    let offset = SOURCE.find(expected).unwrap();
                    assert_eq!(alias.origins[0].start, position(SOURCE, offset));
                }
            }
        }
    }
}

#[test]
fn native_function_and_constructor_annotations_preserve_actual_signature_facts() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, expected, constructor) in [
            ("function", "(x: string) => number", false),
            ("constructor", "new (x: number) => { value: string; }", true),
            ("generic", "<T>(x: T) => T", false),
            (
                "abstract",
                "abstract new (x: number) => { value: string; }",
                true,
            ),
        ] {
            let answer = project.type_of(request(&fixture, marker, true, 3));
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("native anonymous signature must succeed ({marker}): {answer:?}");
            };
            assert_eq!(result.outcome, TypeOutcome::Found);
            assert_eq!(result.description.summary, expected, "{config}/{marker}");
            assert!(result.issues.is_empty());
            let graph = result.description.expansion.unwrap();
            let root = graph
                .nodes
                .iter()
                .find(|node| node.id == graph.root)
                .unwrap();
            let TypeNodeData::Function { members } = &root.data else {
                panic!("actual signature inventory determines Function kind");
            };
            let signatures = if constructor {
                &members.construct_signatures
            } else {
                &members.call_signatures
            };
            let [signature] = signatures.as_ref().unwrap().as_slice() else {
                panic!("one actual anonymous declaration signature is retained");
            };
            assert_eq!(signature.parameters.len(), 1);
            assert_eq!(signature.parameters[0].name, "x");
            if marker == "generic" {
                assert_eq!(signature.type_parameters.len(), 1);
                assert_eq!(signature.parameters[0].value_type, signature.return_type);
            }
            assert!(!graph.truncated);
        }
    }
}

#[test]
fn merged_alias_uses_the_first_canonical_own_written_name() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, decoded, expected, own_names) in [
            (
                "valueFirst",
                "Alias",
                r"\u0041lias",
                [("const ", r"\u0041lias"), ("type ", "Alias")],
            ),
            (
                "aliasFirst",
                "Before",
                r"\u0042efore",
                [("type ", r"\u0042efore"), ("const ", "Before")],
            ),
            (
                "functionFirst",
                "FunctionAlias",
                r"\u0046unctionAlias",
                [
                    ("function ", r"\u0046unctionAlias"),
                    ("type ", "FunctionAlias"),
                ],
            ),
            (
                "plainFirst",
                "Plain",
                "Plain",
                [("type ", "Plain"), ("const ", r"\u0050lain")],
            ),
            (
                "namespaceFirst",
                "ModuleOnly",
                r"\u004doduleOnly",
                [("namespace ", r"\u004doduleOnly"), ("type ", "ModuleOnly")],
            ),
            (
                "namespaceAfter",
                "ModuleBefore",
                r"\u004doduleBefore",
                [
                    ("type ", r"\u004doduleBefore"),
                    ("namespace ", "ModuleBefore"),
                ],
            ),
        ] {
            let prefix = format!("/*#{marker}*/");
            let offset = WRITTEN_NAMES.find(&prefix).unwrap() + prefix.len();
            for (expand, depth) in [(false, 0), (true, 0), (true, 2)] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("written-names.ts"),
                    position: position(WRITTEN_NAMES, offset),
                });
                request.expand = expand;
                request.depth = depth;
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!(
                        "merged alias must complete its native declared type ({marker}): {answer:?}"
                    );
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(
                    result.description.summary, expected,
                    "{config}/{marker}/{depth}"
                );
                assert!(result.issues.is_empty());
                if let Some(graph) = result.description.expansion {
                    let root = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == graph.root)
                        .unwrap();
                    let alias = root.alias.as_ref().unwrap();
                    assert_eq!(alias.name, decoded);
                    let expected_origins: Vec<_> = own_names
                        .iter()
                        .map(|(prefix, raw)| {
                            let start = WRITTEN_NAMES.find(&format!("{prefix}{raw}")).unwrap()
                                + prefix.len();
                            (
                                position(WRITTEN_NAMES, start),
                                position(WRITTEN_NAMES, start + raw.len()),
                            )
                        })
                        .collect();
                    assert_eq!(
                        alias
                            .origins
                            .iter()
                            .map(|origin| (&origin.start, &origin.end))
                            .collect::<Vec<_>>(),
                        expected_origins
                            .iter()
                            .map(|(start, end)| (start, end))
                            .collect::<Vec<_>>()
                    );
                    assert!(
                        alias
                            .origins
                            .iter()
                            .all(|origin| origin.file == "written-names.ts")
                    );
                }
            }
        }
    }
}

#[test]
fn named_targets_and_recursive_functions_share_the_native_written_name_owner() {
    let fixture = fixture();
    for config in ["tsconfig.json", "loose.json"] {
        let project = fixture.load_config(config);
        for (marker, expected) in [
            ("interfaceValueFirst", r"\u0049face"),
            ("interfaceTypeFirst", r"\u0042ack"),
            ("interfaceFunctionFirst", r"\u0043allableNamed"),
            // bindEachStatementFunctionsFirst is observable even when the
            // interface's escaped spelling appears earlier in the text.
            ("interfaceBeforeFunction", "CallableAfter"),
            ("recursiveValue", "() => typeof Recursive"),
        ] {
            let prefix = format!("/*#{marker}*/");
            let offset = WRITTEN_NAMES.find(&prefix).unwrap() + prefix.len();
            for (expand, depth) in [(false, 0), (true, 0), (true, 2)] {
                let mut request = TypeRequest::new(TypeQuery::At {
                    file: fixture.path("written-names.ts"),
                    position: position(WRITTEN_NAMES, offset),
                });
                request.expand = expand;
                request.depth = depth;
                let answer = project.type_of(request);
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("native symbol-written question must succeed ({marker}): {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(
                    result.description.summary, expected,
                    "{config}/{marker}/{depth}"
                );
                assert!(result.issues.is_empty());
            }
        }
        let groups = project
            .find_references(typescript_parser::references::ReferenceQuery::Name {
                name: "Alias".into(),
                file: None,
            })
            .unwrap()
            .groups;
        assert_eq!(
            groups.len(),
            2,
            "the legacy lexical value/type groups remain separate"
        );
        assert_eq!(
            groups
                .iter()
                .map(|group| group.occurrences.len())
                .collect::<Vec<_>>(),
            [1, 2]
        );
    }
}

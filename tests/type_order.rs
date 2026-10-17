//! CompareTypes ordering is visible through the final Project query graph.
mod support;

use support::{Fixture, position};
use typescript_parser::types::{
    TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRef, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/native-type-order/cases.ts");

fn fixture() -> Fixture {
    Fixture::new(&[
        ("cases.ts", SOURCE),
        (
            "first.ts",
            include_str!("../fixtures/types/native-type-order/first.ts"),
        ),
        (
            "second.ts",
            include_str!("../fixtures/types/native-type-order/second.ts"),
        ),
        (
            "tsconfig.json",
            include_str!("../fixtures/types/native-type-order/tsconfig.json"),
        ),
        (
            "reversed.json",
            include_str!("../fixtures/types/native-type-order/reversed.json"),
        ),
    ])
}

fn request(fixture: &Fixture, marker: &str, expand: bool) -> TypeRequest {
    let tag = format!("/*#{marker}*/");
    let mut request = TypeRequest::new(TypeQuery::At {
        file: fixture.path("cases.ts"),
        position: position(SOURCE, SOURCE.find(&tag).unwrap() + tag.len()),
    });
    request.expand = expand;
    request.depth = 1;
    request
}

fn check(cases: &[(&str, &str, &[&str])]) {
    let fixture = fixture();
    for config in ["tsconfig.json", "reversed.json"] {
        for warm in [
            &[][..],
            &["late", "early", "lateFunction", "earlyFunction"][..],
            &["earlyFunction", "lateFunction", "early", "late"][..],
        ] {
            let project = fixture.load_config(config);
            for marker in warm {
                let answer = project.type_of(request(&fixture, marker, false));
                assert!(matches!(answer, Ok(TypeAnswer::Selected(_))), "{answer:?}");
            }
            for &(marker, expected, children) in cases {
                for expand in [false, true, false, true] {
                    let answer = project.type_of(request(&fixture, marker, expand));
                    let Ok(TypeAnswer::Selected(result)) = answer else {
                        panic!(
                            "native ordered union must establish ({config}/{marker}): {answer:?}"
                        );
                    };
                    assert_eq!(result.outcome, TypeOutcome::Found);
                    assert_eq!(
                        result.description.summary, expected,
                        "{config}/{marker}/{warm:?}"
                    );
                    assert!(
                        result.issues.is_empty(),
                        "{config}/{marker}: {:?}",
                        result.issues
                    );
                    if !expand {
                        continue;
                    }
                    let graph = result.description.expansion.unwrap();
                    let root = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == graph.root)
                        .unwrap();
                    let TypeNodeData::Union {
                        alternatives,
                        omitted_alternatives,
                    } = &root.data
                    else {
                        panic!("expected actual union graph ({marker})");
                    };
                    assert_eq!(*omitted_alternatives, None);
                    let actual: Vec<_> = alternatives
                        .iter()
                        .map(|edge| {
                            let TypeRef::Ref { r#ref } = edge else {
                                panic!("native child must establish")
                            };
                            graph
                                .nodes
                                .iter()
                                .find(|node| &node.id == r#ref)
                                .unwrap()
                                .summary
                                .as_str()
                        })
                        .collect();
                    assert_eq!(actual, children, "{config}/{marker}/{warm:?}");
                }
            }
        }
    }
}

#[test]
fn named_types_and_reference_arguments_use_canonical_native_names_and_lists() {
    check(&[
        (
            "names",
            "Alpha | Zebra",
            &["Alpha (cases.ts:2:11)", "Zebra (cases.ts:1:11)"],
        ),
        (
            "arguments",
            "Box<\"a\"> | Box<\"z\">",
            &["Box<\"a\"> (cases.ts:4:11)", "Box<\"z\"> (cases.ts:4:11)"],
        ),
    ]);
}

#[test]
fn intersections_use_constituent_count_then_recursive_native_comparison() {
    check(&[
        (
            "intersectionLists",
            "(Alpha & Zebra) | (Zebra & Alpha)",
            &["Alpha & Zebra", "Zebra & Alpha"],
        ),
        (
            "intersectionLengths",
            "(Gamma & Alpha) | (Zebra & Alpha & Gamma)",
            &["Gamma & Alpha", "Zebra & Alpha & Gamma"],
        ),
    ]);
}

#[test]
fn alias_names_and_arguments_order_actual_constituents_and_denormalized_origins() {
    check(&[
        (
            "aliasNames",
            "AlphaAlias | ZebraAlias",
            &["AlphaAlias (cases.ts:12:6)", "ZebraAlias (cases.ts:11:6)"],
        ),
        (
            "aliasArguments",
            "Choice<\"a\"> | Choice<\"z\">",
            &["\"a\"", "\"z\"", "{ choice: \"a\"; }", "{ choice: \"z\"; }"],
        ),
    ]);
}

#[test]
fn anonymous_and_function_types_use_actual_symbol_declarations() {
    check(&[
        (
            "anonymous",
            "{ early: string; } | { late: number; }",
            &["{ early: string; }", "{ late: number; }"],
        ),
        (
            "functions",
            "(() => string) | (() => number)",
            &[
                "() => string (cases.ts:21:36)",
                "() => number (cases.ts:22:35)",
            ],
        ),
    ]);
}

#[test]
fn anonymous_type_order_tracks_program_file_order() {
    let fixture = fixture();
    for (config, expected) in [
        ("tsconfig.json", "{ first: string; } | { second: number; }"),
        ("reversed.json", "{ second: number; } | { first: string; }"),
    ] {
        let project = fixture.load_config(config);
        let answer = project.type_of(request(&fixture, "fileOrder", false));
        let Ok(TypeAnswer::Selected(result)) = answer else {
            panic!("{answer:?}")
        };
        assert_eq!(result.outcome, TypeOutcome::Found);
        assert_eq!(result.description.summary, expected);
        assert!(result.issues.is_empty());
    }
}

#[test]
fn native_scalar_order_controls_stay_stable() {
    check(&[
        ("numbers", "-2 | 3 | 10", &["-2", "3", "10"]),
        (
            "strings",
            "\"A\" | \"a\" | \"z\"",
            &["\"A\"", "\"a\"", "\"z\""],
        ),
    ]);
    let fixture = fixture();
    let project = fixture.load_config("tsconfig.json");
    let answer = project.type_of(request(&fixture, "booleans", false));
    let Ok(TypeAnswer::Selected(result)) = answer else {
        panic!("{answer:?}")
    };
    assert_eq!(result.description.summary, "boolean");
}

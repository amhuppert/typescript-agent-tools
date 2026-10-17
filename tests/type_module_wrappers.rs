//! Final module properties and summaries pinned by the module-wrapper oracle.
mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

const SOURCE: &str = include_str!("../fixtures/types/native-module-wrappers/cases.ts");
const FILES: &[(&str, &str)] = &[
    ("cases.ts", SOURCE),
    (
        "globals.d.ts",
        include_str!("../fixtures/types/native-module-wrappers/globals.d.ts"),
    ),
    (
        "callable.d.ts",
        include_str!("../fixtures/types/native-module-wrappers/callable.d.ts"),
    ),
    (
        "object.d.ts",
        include_str!("../fixtures/types/native-module-wrappers/object.d.ts"),
    ),
    (
        "default.d.ts",
        include_str!("../fixtures/types/native-module-wrappers/default.d.ts"),
    ),
    (
        "tsconfig.json",
        include_str!("../fixtures/types/native-module-wrappers/tsconfig.json"),
    ),
    (
        "loose.json",
        include_str!("../fixtures/types/native-module-wrappers/loose.json"),
    ),
    (
        "exact.json",
        include_str!("../fixtures/types/native-module-wrappers/exact.json"),
    ),
];

#[test]
fn module_wrappers_keep_actual_default_named_and_optional_values() {
    let fixture = Fixture::new(FILES);
    for config in ["tsconfig", "loose", "exact"] {
        let project = fixture.load_config(&format!("{config}.json"));
        let module = fixture.path("default.d.ts").canonicalize().unwrap();
        let module = module.parent().unwrap().join("default");
        let module_summary = format!(
            "typeof import({})",
            serde_json::to_string(module.to_str().unwrap()).unwrap()
        );
        let object_summary = format!(
            "{{ label: \"object\"; optional?: {}; readonly fixed: 1; }}",
            if config == "tsconfig" {
                "number | undefined"
            } else {
                "number"
            }
        );
        for (name, expected) in [
            ("callableLabel", "\"callable\""),
            ("callableDefault", "typeof run"),
            ("objectLabel", "\"object\""),
            (
                "objectOptional",
                if config == "loose" {
                    "number"
                } else {
                    "number | undefined"
                },
            ),
            ("objectFixed", "1"),
            ("realDefault", "() => \"default\""),
            ("realLabel", "\"real\""),
            ("callableNamespace", "typeof run"),
            ("objectNamespace", object_summary.as_str()),
            ("realNamespace", module_summary.as_str()),
        ] {
            let marker = format!("/*#{name}*/");
            for (expand, depth) in [(false, 0), (false, 2), (true, 0), (true, 2)] {
                let result = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(SOURCE, SOURCE.find(&marker).unwrap() + marker.len()),
                    },
                    expand,
                    depth,
                });
                let Ok(TypeAnswer::Selected(answer)) = result else {
                    panic!("{config}/{name}/{expand}/{depth}: {result:?}")
                };
                assert_eq!(
                    answer.outcome,
                    TypeOutcome::Found,
                    "{config}/{name}/{expand}/{depth}: {:?}",
                    answer.issues
                );
                assert_eq!(
                    answer.description.summary, expected,
                    "{config}/{name}/{expand}/{depth}"
                );
                assert!(
                    answer.issues.is_empty(),
                    "{config}/{name}/{expand}/{depth}: {:?}",
                    answer.issues
                );
                if expand {
                    let graph = answer
                        .description
                        .expansion
                        .as_ref()
                        .expect("expanded request retains graph");
                    assert!(graph.nodes.iter().any(|node| node.id == graph.root));
                    assert!(
                        graph
                            .nodes
                            .iter()
                            .all(|node| !matches!(&node.data, TypeNodeData::Unsupported { .. })),
                        "{config}/{name}/{depth}: {graph:?}"
                    );
                    if depth == 2
                        && matches!(
                            name,
                            "callableDefault"
                                | "callableNamespace"
                                | "objectNamespace"
                                | "realNamespace"
                        )
                    {
                        assert!(
                            graph.nodes.len() > 1,
                            "{config}/{name}: expanded members retain actual child edges: {graph:?}"
                        );
                    }
                } else {
                    assert!(answer.description.expansion.is_none());
                }
            }
        }
    }
}

mod support;
use support::{Fixture, position};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn wide_tuple_prefix_reports_every_unshown_established_element() {
    for count in [3_000, 5_000] {
        let elements = vec!["number"; count].join(",");
        let source = format!(
            "type Wide = [{elements}];\ninterface Array<T> {{ length: number; [index: number]: T }}\ninterface ReadonlyArray<T> {{ readonly length: number; readonly [index: number]: T }}\ndeclare const value: Wide;\n"
        );
        for (strict, exact) in [(false, false), (true, false), (true, true)] {
            let config = format!(
                "{{\"compilerOptions\":{{\"strict\":{strict},\"exactOptionalPropertyTypes\":{exact},\"noLib\":true}},\"files\":[\"cases.ts\"]}}"
            );
            let fixture = Fixture::new(&[("cases.ts", &source), ("tsconfig.json", &config)]);
            let project = fixture.load();
            for depth in [0, 1] {
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path("cases.ts"),
                        position: position(&source, source.rfind("value:").unwrap()),
                    },
                    expand: true,
                    depth,
                });
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{strict}/{exact}/{depth}: {answer:?}");
                };
                assert_eq!(result.outcome, TypeOutcome::Found);
                assert_eq!(result.description.summary, "Wide");
                assert!(result.issues.is_empty());
                assert!(serde_json::to_vec(&result.description).unwrap().len() <= 262_144);
                let graph = result.description.expansion.unwrap();
                assert!(graph.truncated);
                let root = &graph.nodes[0];
                let TypeNodeData::Tuple {
                    elements,
                    omitted_elements,
                    ..
                } = &root.data
                else {
                    panic!("{root:?}");
                };
                assert_eq!(
                    *omitted_elements,
                    (elements.len() < count).then_some(count - elements.len())
                );
                if depth == 0 {
                    assert!(elements.is_empty());
                    assert_eq!(root.omitted_type_arguments, Some(count));
                    assert!(root.type_arguments.is_none());
                } else {
                    assert!(!elements.is_empty());
                    if count == 3_000 {
                        assert_eq!(elements.len(), count);
                    } else {
                        assert!(elements.len() < count);
                    }
                    assert_eq!(root.omitted_type_arguments, None);
                    let arguments = root.type_arguments.as_ref().unwrap();
                    assert_eq!(arguments.len(), count);
                    assert!(arguments.iter().all(|argument| argument == &arguments[0]));
                    assert!(
                        elements
                            .iter()
                            .all(|element| element.value_type == arguments[0])
                    );
                }
                assert_eq!(
                    root.omitted.as_ref().is_some_and(|omitted| omitted
                        .fields
                        .iter()
                        .any(|field| field == "elements")),
                    elements.len() < count
                );
            }
        }
    }
}

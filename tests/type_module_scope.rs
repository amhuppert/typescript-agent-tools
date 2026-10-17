//! Native resolveNameHelper visits raw module exports after lexical locals.
//! The maintained compiler rows distinguish actual exported declarations from
//! pure re-exports, whose same-named globals must remain visible.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::InputErrorKind;
use typescript_parser::project::Project;
use typescript_parser::references::{Location, ReferenceQuery};
use typescript_parser::types::{
    PrimitiveName, TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest,
};

#[test]
fn module_scope_uses_native_export_eligibility_and_exact_names() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-module-scope"),
    );
    for profile in ["strict", "loose"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 23);
        for reverse in [false, true] {
            let project = fixture.load_config(&format!("{profile}.json"));
            let mut ordered: Vec<_> = rows.iter().collect();
            if reverse {
                ordered.reverse();
            }
            for row in ordered {
                let file = row["file"].as_str().unwrap();
                let name = row["name"].as_str().unwrap();
                let source = std::fs::read_to_string(fixture.path(file)).unwrap();
                let marker = format!("/*#{name}*/");
                let at = source.find(&marker).unwrap() + marker.len();
                let expected = row["summary"].as_str().unwrap();
                assert_eq!(row["errorType"], false, "{profile}/{name}");
                assert!(
                    row["codes"].as_array().unwrap().is_empty(),
                    "{profile}/{name}"
                );
                assert!(
                    row["demandFailures"].as_array().unwrap().is_empty(),
                    "{profile}/{name}"
                );
                for expand in [false, true] {
                    let label = format!("{profile}/{name}/{expand}/{reverse}");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path(file),
                            position: position(&source, at),
                        },
                        expand,
                        depth: 2,
                    });
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{label}: {answer:?}");
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found, "{label}: {answer:?}");
                    assert_eq!(answer.description.summary, expected, "{label}");
                    assert!(!answer.description.summary_truncated, "{label}");
                    assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                    if expand {
                        let graph = answer.description.expansion.unwrap();
                        assert!(!graph.truncated, "{label}");
                        assert_eq!(graph.nodes.len(), 1, "{label}");
                        assert_eq!(
                            graph.nodes[0].data,
                            TypeNodeData::Literal {
                                base: PrimitiveName::Number,
                                value: expected.into(),
                            },
                            "{label}"
                        );
                        assert!(graph.nodes[0].omitted.is_none(), "{label}");
                    } else {
                        assert!(answer.description.expansion.is_none(), "{label}");
                    }
                }
            }
        }
    }
}

#[test]
fn merged_module_scope_keeps_native_types_and_documented_reference_merges() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-module-scope"),
    );
    for profile in ["strict", "loose"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-boundaries-{profile}.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 9);
        for order in ["forward", "reverse"] {
            let config = format!("merged-{order}-{profile}.json");
            let project = fixture.load_config(&config);
            for row in rows.iter().filter(|row| row["config"] == config) {
                let file = row["file"].as_str().unwrap();
                let source = std::fs::read_to_string(fixture.path(file)).unwrap();
                let marker = format!("/*#{}*/", row["name"].as_str().unwrap());
                let at = source.find(&marker).unwrap() + marker.len();
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path(file),
                            position: position(&source, at),
                        },
                        expand,
                        depth: 2,
                    });
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{config}: {answer:?}");
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found, "{config}: {answer:?}");
                    assert_eq!(answer.description.summary, row["summary"].as_str().unwrap());
                    assert!(!answer.description.summary_truncated);
                    assert!(answer.issues.is_empty());
                    if expand {
                        let graph = answer.description.expansion.unwrap();
                        assert!(!graph.truncated);
                        assert_eq!(graph.nodes.len(), 1);
                        assert_eq!(
                            graph.nodes[0].data,
                            TypeNodeData::Literal {
                                base: PrimitiveName::Number,
                                value: row["summary"].as_str().unwrap().into(),
                            }
                        );
                        assert!(graph.nodes[0].omitted.is_none());
                    }
                }
            }
            let references: Vec<serde_json::Value> = serde_json::from_str(
                &std::fs::read_to_string(
                    fixture.path(&format!("expected-{order}-{profile}-references.json")),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(references.len(), 7);
            for row in references {
                let site: Location = serde_json::from_value(row["location"].clone()).unwrap();
                let answer = project
                    .find_references(ReferenceQuery::At {
                        file: fixture.path(&site.file),
                        position: site.start,
                    })
                    .unwrap_or_else(|error| panic!("{config}/{site:?}: {error:?}"));
                assert_eq!(
                    serde_json::to_value(answer.groups).unwrap(),
                    merged_reference_expectation(&row, order),
                    "{config}/{site:?}"
                );
            }
        }
    }
}

fn merged_reference_expectation(row: &serde_json::Value, order: &str) -> serde_json::Value {
    let mut expected = row["groups"].clone();
    let site = &row["location"];
    if order != "forward"
        || site["file"] != "merged-b.d.ts"
        || ![(5, 22), (6, 38)].iter().any(|&(line, column)| {
            site["start"]["line"] == line && site["start"]["column"] == column
        })
    {
        return expected;
    }

    // docs/references.md, "Differences from TypeScript": aliases follow the
    // merged identity here. Native mergeSymbol clones the resolved target,
    // but resolveAlias can keep its cached unmerged target. The maintained
    // pattern-export-merge exception and reference_augmentation's
    // merges_through_aliases_follow_the_declaration_meanings cover this rule.
    // Alias-before-interface merges through the alias target. In reverse
    // order the source alias is attached to the existing interface, adding no
    // new meaning to propagate (the ordered merge in docs/references.md).
    // Both profiles keep native Q4 expectations; reverse Q3 stays native too.
    // Only forward Q3 gains the two aliases' four export tokens.
    assert_eq!(expected.as_array().unwrap().len(), 1);
    assert_eq!(expected[0]["declarations"].as_array().unwrap().len(), 1);
    assert_eq!(
        expected[0]["declarations"][0]["location"]["file"],
        "merged-b.d.ts"
    );
    assert_eq!(
        expected[0]["declarations"][0]["location"]["start"],
        serde_json::json!({ "line": 5, "column": 22 })
    );
    let native_occurrences = expected[0]["occurrences"].as_array().unwrap();
    assert_eq!(native_occurrences.len(), 2);
    let mut occurrences: Vec<_> = [
        ("value", 14, 19),
        ("MixedEntry", 23, 33),
        ("value", 35, 40),
        ("PureEntry", 44, 53),
    ]
    .into_iter()
    .map(|(name, start, end)| {
        serde_json::json!({
            "name": name,
            "location": {
                "file": "merged-a.d.ts",
                "start": { "line": 3, "column": start },
                "end": { "line": 3, "column": end },
            },
            "roles": ["export"],
        })
    })
    .collect();
    occurrences.extend(native_occurrences.iter().cloned());
    expected[0]["occurrences"] = occurrences.into();
    expected
}

#[test]
fn module_scope_name_failure_is_a_located_load_error_only_when_observed() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-module-scope"),
    );
    for profile in ["strict", "loose"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(fixture.path(&format!("expected-boundaries-{profile}.json")))
                .unwrap(),
        )
        .unwrap();
        for row in rows
            .iter()
            .filter(|row| row["config"].as_str().unwrap().starts_with("name-"))
        {
            assert_eq!(row["summary"], "91");
            assert_eq!(row["errorType"], false);
            assert!(row["codes"].as_array().unwrap().is_empty());
            assert!(row["demandFailures"].as_array().unwrap().is_empty());
            let file = row["file"].as_str().unwrap();
            let source = std::fs::read_to_string(fixture.path(file)).unwrap();
            let loaded = Project::load(&fixture.path(row["config"].as_str().unwrap()))
                .map_err(support::input_error);
            if file == "name-failure.ts" {
                let error = loaded
                    .expect_err("unavailable raw export names must prevent a successful snapshot");
                assert_eq!(error.kind, InputErrorKind::Unsupported);
                assert_eq!(
                    error.message,
                    "native name lookup could not complete: native symbol name contains an unrepresentable UTF-16 identity"
                );
                assert_eq!(error.path, Some(fixture.path(file)));
                assert_eq!(
                    error.position,
                    Some(position(&source, source.rfind("Outside").unwrap()))
                );
            } else {
                let project = loaded.unwrap_or_else(|error| panic!("{profile}/{file}: {error:?}"));
                let marker = format!("/*#{}*/", row["name"].as_str().unwrap());
                let at = source.find(&marker).unwrap() + marker.len();
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path(file),
                        position: position(&source, at),
                    },
                    expand: true,
                    depth: 2,
                });
                let Ok(TypeAnswer::Selected(answer)) = answer else {
                    panic!("{profile}/{file}: {answer:?}");
                };
                assert_eq!(
                    answer.outcome,
                    TypeOutcome::Found,
                    "{profile}/{file}: {answer:?}"
                );
                assert_eq!(answer.description.summary, "91");
                assert!(answer.issues.is_empty());
                let graph = answer.description.expansion.unwrap();
                assert!(!graph.truncated);
                assert_eq!(graph.nodes.len(), 1);
                assert_eq!(
                    graph.nodes[0].data,
                    TypeNodeData::Literal {
                        base: PrimitiveName::Number,
                        value: "91".into()
                    }
                );
            }
        }
    }
}

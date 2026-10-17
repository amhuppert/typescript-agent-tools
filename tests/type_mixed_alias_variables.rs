//! Native Alias and Variable flags coexist on the same actual symbol header.
//! The compiler fixture records both flags and completed flow identities.
mod support;

use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{
    TypeAnswer, TypeKey, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest,
};

const SOURCE: &str = include_str!("../fixtures/types/native-mixed-alias-variables/cases.ts");

fn check(markers: &[(&str, Option<&str>)]) {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-mixed-alias-variables"),
    );
    for profile in ["tsconfig.json", "loose.json"] {
        for reverse in [false, true] {
            let project = fixture.load_config(profile);
            let mut ordered = markers.to_vec();
            if reverse {
                ordered.reverse();
            }
            for (marker, expected) in ordered {
                let tag = format!("/*#{marker}*/");
                let offset = SOURCE.find(&tag).unwrap() + tag.len();
                for expand in [false, true] {
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: fixture.path("cases.ts"),
                            position: position(SOURCE, offset),
                        },
                        expand,
                        depth: 1,
                    });
                    match expected {
                        Some(summary) => {
                            let Ok(TypeAnswer::Selected(answer)) = answer else {
                                panic!("{profile}/{marker}/{expand}: {answer:?}");
                            };
                            assert_eq!(answer.outcome, TypeOutcome::Found);
                            assert_eq!(answer.description.summary, summary, "{profile}/{marker}");
                            assert!(answer.issues.is_empty());
                            if let Some(graph) = answer.description.expansion {
                                assert!(
                                    graph.nodes.iter().all(|node| {
                                        !matches!(node.data, TypeNodeData::Unsupported { .. })
                                    }),
                                    "{profile}/{marker}: {graph:?}"
                                );
                            }
                        }
                        None => {
                            let Err(AnalysisError::Semantic(error)) = answer else {
                                panic!("{profile}/{marker}/{expand}: {answer:?}");
                            };
                            assert_eq!(error.kind, SemanticErrorKind::UnsupportedSemantic);
                            assert_eq!(error.reason, SemanticFailureReason::FlowRequired.into());
                            assert!(
                                error
                                    .message
                                    .contains("native reference flow changes this read"),
                                "the completed native refinement, rather than a missing mixed-header attachment, must own the failure: {profile}/{marker}: {error:?}"
                            );
                            assert_eq!(
                                error.path.as_deref(),
                                Some(fixture.path("cases.ts").as_path())
                            );
                            let failure_offset = if marker.ends_with("Element") {
                                offset + SOURCE[offset..].find("records[").unwrap()
                            } else {
                                offset
                            };
                            assert_eq!(
                                error.position,
                                Some(position(SOURCE, failure_offset)),
                                "{profile}/{marker}: {error:?}"
                            );
                            assert!(error.causes.is_empty(), "{profile}/{marker}: {error:?}");
                            assert!(error.omitted_causes.is_none());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn mixed_variable_reads_keep_local_family_and_alias_initialization_in_both_orders() {
    check(&[
        ("firstVar", Some("number")),
        ("firstLet", Some("number")),
        ("firstConst", Some("number")),
        ("firstUninitialized", Some("number")),
        ("lastVar", Some("number")),
        ("lastLet", Some("number")),
        ("lastConst", Some("number")),
        ("lastUninitialized", Some("number")),
        ("directInitialized", Some("number")),
    ]);
}

#[test]
fn mixed_variable_assignments_keep_native_capture_boundaries() {
    check(&[
        ("captureBefore", Some("string | number")),
        ("captureAfter", None),
        ("nestedAssignment", Some("string | number")),
        ("exportedAssignment", Some("string | number")),
    ]);
}

#[test]
fn mixed_variable_element_keys_distinguish_constants_unassigned_and_assigned_locals() {
    check(&[
        ("stableConstElement", None),
        ("stableLetElement", None),
        ("assignedLetElement", Some("string | number")),
    ]);
}

#[test]
fn mixed_variable_alias_conditions_use_actual_assignment_facts() {
    check(&[
        ("mutableAliasCondition", None),
        ("assignedAliasCondition", Some("string | number")),
    ]);
}

fn check_module_queries(
    config: &str,
    file: &str,
    source: &str,
    markers: &[(&str, Option<&str>, Option<&str>)],
) {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-mixed-alias-variables"),
    );
    for reverse in [false, true] {
        let project = fixture.load_config(config);
        let mut ordered = markers.to_vec();
        if reverse {
            ordered.reverse();
        }
        for (marker, summary, property) in ordered {
            let tag = format!("/*#{marker}*/");
            let offset = source.find(&tag).unwrap() + tag.len();
            for expand in [false, true] {
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path(file),
                        position: position(source, offset),
                    },
                    expand,
                    depth: 1,
                });
                let Some(summary) = summary else {
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{config}/{marker}/{expand}: {answer:?}");
                    };
                    assert_eq!(error.kind, SemanticErrorKind::InvalidSemantic);
                    assert_eq!(error.reason, SemanticFailureReason::UnresolvedSymbol.into());
                    assert!(
                        error
                            .message
                            .contains("namespace has no exported member `Absent`"),
                        "{error:?}"
                    );
                    assert_eq!(error.path.as_deref(), Some(fixture.path(file).as_path()));
                    assert_eq!(
                        error.position,
                        Some(position(
                            source,
                            source.find("Combined.Absent").unwrap() + "Combined.".len()
                        ))
                    );
                    assert!(error.causes.is_empty(), "{error:?}");
                    assert!(error.omitted_causes.is_none());
                    continue;
                };
                let Ok(TypeAnswer::Selected(answer)) = answer else {
                    panic!("{config}/{marker}/{expand}: {answer:?}");
                };
                assert_eq!(answer.outcome, TypeOutcome::Found);
                assert_eq!(answer.description.summary, summary, "{config}/{marker}");
                assert!(answer.issues.is_empty());
                if let Some(graph) = answer.description.expansion {
                    assert!(
                        graph
                            .nodes
                            .iter()
                            .all(|node| { !matches!(node.data, TypeNodeData::Unsupported { .. }) }),
                        "{config}/{marker}: {graph:?}"
                    );
                    if let Some(property) = property {
                        let root = graph
                            .nodes
                            .iter()
                            .find(|node| node.id == graph.root)
                            .unwrap();
                        let TypeNodeData::Object { members } = &root.data else {
                            panic!("{config}/{marker}: {root:?}");
                        };
                        let properties = members.properties.as_ref().unwrap();
                        assert_eq!(properties.len(), 1, "{config}/{marker}");
                        assert_eq!(
                            properties[0].key,
                            TypeKey::String {
                                name: property.into()
                            }
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn merged_namespace_looks_through_alias_only_after_own_member_absence() {
    check_module_queries(
        "namespaces.json",
        "namespace-cases.ts",
        include_str!("../fixtures/types/native-mixed-alias-variables/namespace-cases.ts"),
        &[
            ("namespaceLocal", Some("Local"), Some("local")),
            ("namespaceAliasFallback", Some("Remote"), Some("remote")),
            ("namespaceOwnWins", Some("\"local\""), None),
            ("namespaceMissing", None, None),
        ],
    );
}

#[test]
fn merged_namespace_retries_alias_when_own_name_has_another_meaning() {
    check_module_queries(
        "namespaces.json",
        "namespace-cases.ts",
        include_str!("../fixtures/types/native-mixed-alias-variables/namespace-cases.ts"),
        &[(
            "namespaceMeaningFallback",
            Some("TypeMember"),
            Some("remote"),
        )],
    );
}

#[test]
fn repeated_star_module_augmentations_do_not_warm_the_module_exports_cache() {
    check_module_queries(
        "augmentations.json",
        "augmentation-cases.ts",
        include_str!("../fixtures/types/native-mixed-alias-variables/augmentation-cases.ts"),
        &[
            ("plainRemote", Some("Remote"), Some("remote")),
            ("plainFirst", Some("First"), Some("first")),
            ("plainSecond", Some("Second"), Some("second")),
            ("typeRemote", Some("Remote"), Some("remote")),
            ("typeFirst", Some("First"), Some("first")),
            ("typeSecond", Some("Second"), Some("second")),
        ],
    );
}

//! Initialization and ordinary queries share the native JSON and merge owners.
//! A clone's first ValueDeclaration, global-array recovery, and unrelated queries
//! remain distinct even when initialization eagerly resolves an export alias.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeNodeData, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn initialized_json_aliases_keep_native_types_origins_and_request_isolation() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-json-initialization");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(root.join("cases.json")).unwrap()).unwrap();
    assert_eq!(cases.len(), 12);
    for profile in ["strict", "loose"] {
        let config = std::fs::read_to_string(root.join(format!("{profile}.json"))).unwrap();
        let expected: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-{profile}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(expected.len(), cases.len());
        let expected_properties: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-properties-{profile}.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(expected_properties.len(), cases.len());
        for (case, native) in cases.iter().zip(&expected) {
            assert_eq!(case["name"], native["name"]);
            let name = case["name"].as_str().unwrap();
            let properties = expected_properties
                .iter()
                .find(|row| row["name"] == name)
                .unwrap();
            let mut files: Vec<_> = case["files"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(name, text)| (name.as_str(), text.as_str().unwrap()))
                .collect();
            files.push(("tsconfig.json", &config));
            let fixture = Fixture::new(&files);
            let source = case["files"]["main.ts"].as_str().unwrap();
            for reverse in [false, true] {
                let project = fixture.load();
                let mut queries: Vec<_> = native["queries"].as_array().unwrap().iter().collect();
                if reverse {
                    queries.reverse();
                }
                for row in queries {
                    let query = row["name"].as_str().unwrap();
                    let marker = format!("/*#{query}*/");
                    let at = position(source, source.find(&marker).unwrap() + marker.len());
                    assert_eq!(row["demandFailures"], serde_json::json!([]));
                    for expand in [false, true, false] {
                        let label = format!("{profile}/{name}/{query}/{expand}/{reverse}");
                        let answer = project.type_of(TypeRequest {
                            query: TypeQuery::At {
                                file: fixture.path("main.ts"),
                                position: at,
                            },
                            expand,
                            depth: 1,
                        });
                        let consumes_array_recovery = query != "healthy"
                            && matches!(name, "array-missing" | "array-wrong-arity");
                        if consumes_array_recovery {
                            // Native stores emptyGenericType (displayed as {}) for
                            // these global failures. The product contract preserves
                            // consumed factory recovery, without poisoning healthy.
                            assert_eq!(row["summary"], "{}");
                            assert_eq!(row["errorType"], false);
                            let Err(AnalysisError::Semantic(error)) = answer else {
                                panic!("{label}: {answer:?}");
                            };
                            assert_eq!(
                                error.kind,
                                SemanticErrorKind::InvalidSemantic,
                                "{label}: {error:?}"
                            );
                            assert_eq!(
                                error.reason,
                                SemanticFailureReason::ErrorType.into(),
                                "{label}: {error:?}"
                            );
                            assert_eq!(
                                error.path,
                                Some(fixture.path("data.json")),
                                "{label}: {error:?}"
                            );
                            let json = case["files"]["data.json"].as_str().unwrap();
                            assert_eq!(
                                error.position,
                                Some(position(json, json.find('[').unwrap())),
                                "{label}: {error:?}"
                            );
                            let missing = name == "array-missing";
                            let message = if missing {
                                "Cannot find global type 'Array'."
                            } else {
                                "Global type 'Array' must have 1 type parameter(s)."
                            };
                            let cause = error
                                .causes
                                .iter()
                                .find(|cause| cause.message == message)
                                .unwrap_or_else(|| {
                                    panic!("{label}: missing global cause: {error:?}")
                                });
                            assert_eq!(
                                cause.reason,
                                if missing {
                                    SemanticFailureReason::UnresolvedSymbol.into()
                                } else {
                                    SemanticFailureReason::ErrorType.into()
                                },
                                "{label}: {error:?}"
                            );
                            if missing {
                                assert!(cause.location.is_none(), "{label}: {error:?}");
                            } else {
                                let globals = case["files"]["globals.d.ts"].as_str().unwrap();
                                let start =
                                    globals.find("interface Array").unwrap() + "interface ".len();
                                let location = cause.location.as_ref().unwrap();
                                assert_eq!(location.file, "globals.d.ts");
                                assert_eq!(location.start, position(globals, start));
                                assert_eq!(location.end, position(globals, start + "Array".len()));
                            }
                            assert!(error.omitted_causes.is_none(), "{label}: {error:?}");
                        } else if row["errorType"] == true {
                            assert_eq!((name, query), ("duplicate-last-object", "detail"));
                            let Err(AnalysisError::Semantic(error)) = answer else {
                                panic!("{label}: {answer:?}");
                            };
                            assert_eq!(
                                error.kind,
                                SemanticErrorKind::InvalidSemantic,
                                "{label}: {error:?}"
                            );
                            assert_eq!(
                                error.reason,
                                SemanticFailureReason::ErrorType.into(),
                                "{label}: {error:?}"
                            );
                            assert_eq!(
                                error.path,
                                Some(fixture.path("main.ts")),
                                "{label}: {error:?}"
                            );
                            let access_start = source.find("X./*#detail*/last").unwrap();
                            assert_eq!(
                                error.position,
                                Some(position(source, access_start)),
                                "{label}: {error:?}"
                            );
                            let cause = error
                                .causes
                                .iter()
                                .find(|cause| {
                                    cause.message
                                        == "property and applicable index signature are absent"
                                })
                                .unwrap_or_else(|| {
                                    panic!("{label}: missing property cause: {error:?}")
                                });
                            let location = cause.location.as_ref().unwrap();
                            assert_eq!(location.file, "main.ts");
                            assert_eq!(location.start, at, "{label}");
                            assert_eq!(
                                location.end,
                                position(source, source.find("last;").unwrap() + "last".len()),
                                "{label}"
                            );
                        } else {
                            let Ok(TypeAnswer::Selected(answer)) = answer else {
                                panic!("{label}: {answer:?}");
                            };
                            assert_eq!(answer.outcome, TypeOutcome::Found, "{label}: {answer:?}");
                            assert_eq!(
                                answer.description.summary,
                                row["summary"].as_str().unwrap(),
                                "{label}"
                            );
                            assert!(answer.issues.is_empty(), "{label}: {answer:?}");
                            assert_eq!(
                                answer.description.expansion.is_some(),
                                expand,
                                "{label}: expansion presence"
                            );
                            if let Some(graph) = answer.description.expansion {
                                assert!(
                                    graph.nodes.iter().all(|node| !matches!(
                                        node.data,
                                        TypeNodeData::Unsupported { .. }
                                    )),
                                    "{label}: {graph:?}"
                                );
                                if query == "alias"
                                    && !properties["properties"].as_array().unwrap().is_empty()
                                {
                                    let root = graph
                                        .nodes
                                        .iter()
                                        .find(|node| node.id == graph.root)
                                        .unwrap();
                                    let members = match &root.data {
                                        TypeNodeData::Object { members }
                                        | TypeNodeData::Array { members, .. } => members,
                                        _ => panic!("{label}: expected native object: {root:?}"),
                                    };
                                    let actual_properties = members.properties.as_ref().unwrap();
                                    let actual: Vec<_> = actual_properties.iter().map(|property| {
                                        let typescript_parser::types::TypeKey::String { name } = &property.key else {
                                            panic!("{label}: unexpected key {:?}", property.key);
                                        };
                                        serde_json::json!({"name": name, "origins": property.origins})
                                    }).collect();
                                    assert_eq!(
                                        serde_json::json!(actual),
                                        properties["properties"],
                                        "{label}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn initialized_json_member_references_preserve_origins_across_fresh_queries() {
    use typescript_parser::references::{
        ReferenceAnswer, ReferenceBaseline, ReferenceFilters, ReferenceOutcome, ReferenceQuery,
        ReferenceRequest,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-json-initialization");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(root.join("cases.json")).unwrap()).unwrap();
    for profile in ["strict", "loose"] {
        let config = std::fs::read_to_string(root.join(format!("{profile}.json"))).unwrap();
        let expected: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-members-{profile}.json")))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(expected.len(), 6);
        for row in expected {
            let name = row["name"].as_str().unwrap();
            let case = cases.iter().find(|case| case["name"] == name).unwrap();
            let mut files: Vec<_> = case["files"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(name, text)| (name.as_str(), text.as_str().unwrap()))
                .collect();
            files.push(("tsconfig.json", &config));
            let fixture = Fixture::new(&files);
            let source = case["files"]["main.ts"].as_str().unwrap();
            let tag = "/*#detail*/";
            let at = position(source, source.find(tag).unwrap() + tag.len());
            let healthy_tag = "/*#healthy*/";
            let healthy_at = position(
                source,
                source.find(healthy_tag).unwrap() + healthy_tag.len(),
            );
            for reverse in [false, true] {
                let project = fixture.load();
                // Each request owns its checker. Interleave Q3 with a Q4 demand
                // that must stay independent of initialization's cached values.
                for member in if reverse {
                    [false, true, false, true]
                } else {
                    [true, false, true, false]
                } {
                    let label = format!("{profile}/{name}/{reverse}/{member}");
                    if member {
                        let answer = project.references(ReferenceRequest {
                            query: ReferenceQuery::At {
                                file: fixture.path("main.ts"),
                                position: at,
                            },
                            filters: ReferenceFilters::default(),
                        });
                        let Ok(ReferenceAnswer::Selected(answer)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(
                            answer.outcome,
                            ReferenceOutcome::Complete,
                            "{label}: {answer:?}"
                        );
                        assert_eq!(
                            answer.coverage.baseline,
                            ReferenceBaseline::Member,
                            "{label}"
                        );
                        assert!(answer.coverage.gaps.is_empty(), "{label}: {answer:?}");
                        let actual: Vec<_> = answer.groups.iter().map(|group| serde_json::json!({
                            "origins": group.declarations.iter().map(|declaration| &declaration.location).collect::<Vec<_>>(),
                            "occurrences": group.occurrences.iter().map(|occurrence| &occurrence.location).collect::<Vec<_>>(),
                        })).collect();
                        let expected: Vec<_> = row["groups"].as_array().unwrap().iter().map(|group| serde_json::json!({
                            "origins": group["origins"].as_array().unwrap().iter().map(|origin| &origin["location"]).collect::<Vec<_>>(),
                            "occurrences": group["occurrences"],
                        })).collect();
                        assert_eq!(actual, expected, "{label}");
                        let filtered = project
                            .references(ReferenceRequest {
                                query: ReferenceQuery::At {
                                    file: fixture.path("main.ts"),
                                    position: at,
                                },
                                filters: ReferenceFilters {
                                    calls: true,
                                    exclude_tests: false,
                                    file: None,
                                },
                            })
                            .unwrap();
                        let ReferenceAnswer::Selected(filtered) = filtered else {
                            panic!("{label}: {filtered:?}");
                        };
                        assert_eq!(
                            filtered.outcome,
                            ReferenceOutcome::Complete,
                            "{label}: {filtered:?}"
                        );
                        assert_eq!(
                            filtered.groups.len(),
                            1,
                            "{label}: retain one native seed group"
                        );
                        assert!(
                            filtered.groups[0].occurrences.is_empty(),
                            "{label}: {filtered:?}"
                        );
                        assert_eq!(
                            filtered.groups[0].declarations, answer.groups[0].declarations,
                            "{label}"
                        );
                    } else {
                        let answer = project.type_of(TypeRequest::new(TypeQuery::At {
                            file: fixture.path("main.ts"),
                            position: healthy_at,
                        }));
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{label}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found);
                        assert_eq!(answer.description.summary, "44");
                        assert!(answer.issues.is_empty());
                    }
                }
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn shared_json_initialization_preserves_load_failures_and_service_generations() {
    use support::service::{Service, json};
    use typescript_parser::project::Project;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-json-initialization");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(root.join("cases.json")).unwrap()).unwrap();
    let case = cases
        .iter()
        .find(|case| case["name"] == "own-direct")
        .unwrap();
    for profile in ["strict", "loose"] {
        let config = std::fs::read_to_string(root.join(format!("{profile}.json"))).unwrap();
        let mut files: Vec<_> = case["files"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, text)| (name.as_str(), text.as_str().unwrap()))
            .collect();
        files.push(("tsconfig.json", &config));
        let service = Service::of(Fixture::new(&files));
        let source = case["files"]["main.ts"].as_str().unwrap();
        let alias_marker = "/*#alias*/";
        let alias = position(
            source,
            source.find(alias_marker).unwrap() + alias_marker.len(),
        );
        let alias_at = format!("main.ts:{}:{}", alias.line, alias.column);
        let requests: &[&[&str]] = &[
            &[
                "--type", "--at", &alias_at, "--expand", "--depth", "1", "--json",
            ],
            &["--references", "--at", "main.ts:2:14", "--json"],
            &["--type", "--name", "healthy", "main.ts", "--json"],
        ];
        for (index, request) in requests.iter().enumerate() {
            let direct = service.run(&[request.to_vec(), vec!["--direct"]].concat());
            let remote = service.run(request);
            assert_eq!(
                remote.status.code(),
                Some(0),
                "{profile}/{index}: {remote:?}"
            );
            assert_eq!(remote.status.code(), direct.status.code());
            assert_eq!(remote.stdout, direct.stdout, "{profile}/{index}");
            assert_eq!(remote.stderr, direct.stderr, "{profile}/{index}");
            let document = json(remote);
            match index {
                0 => assert_eq!(document["type"]["summary"], "{ nested: number; }"),
                1 => {
                    assert_eq!(document["groups"].as_array().unwrap().len(), 1);
                    assert_eq!(
                        document["groups"][0]["occurrences"]
                            .as_array()
                            .unwrap()
                            .len(),
                        2
                    );
                }
                2 => assert_eq!(document["type"]["summary"], "44"),
                _ => unreachable!(),
            }
        }
        assert_eq!(service.generation(), 1);

        // The augmentation demands the entire JSON value during loading. A
        // non-scalar identity is a product UnsupportedSemantic failure, never
        // a tolerated dependency InputError or a stale healthy query result.
        let invalid = r#"{"a":{"nested":1},"\ud800":0}"#;
        std::fs::write(service.fixture.path("data.json"), invalid).unwrap();
        let failure = Project::load(&service.fixture.path("tsconfig.json")).unwrap_err();
        let AnalysisError::Semantic(error) = failure else {
            panic!("{profile}: {failure:?}");
        };
        assert_eq!(
            error.kind,
            SemanticErrorKind::UnsupportedSemantic,
            "{profile}: {error:?}"
        );
        assert_eq!(
            error.reason,
            SemanticFailureReason::UnrepresentableIdentity.into()
        );
        assert_eq!(error.path, Some(service.fixture.path("data.json")));
        assert_eq!(
            error.position,
            Some(position(invalid, invalid.find(r#""\ud800""#).unwrap()))
        );
        for _ in 0..2 {
            for request in requests {
                let direct = service.run(&[request.to_vec(), vec!["--direct"]].concat());
                let remote = service.run(request);
                assert_eq!(remote.status.code(), Some(1), "{profile}: {remote:?}");
                assert_eq!(remote.status.code(), direct.status.code());
                assert_eq!(remote.stdout, direct.stdout, "{profile}");
                assert_eq!(remote.stderr, direct.stderr, "{profile}");
                assert!(remote.stderr.is_empty());
                let document: serde_json::Value = serde_json::from_slice(&remote.stdout).unwrap();
                assert_eq!(document["outcome"], "error");
                assert_eq!(
                    document["error"]["kind"],
                    serde_json::json!(SemanticErrorKind::UnsupportedSemantic)
                );
                assert_eq!(document["error"]["reason"], "unrepresentable_identity");
                assert_eq!(document["error"]["message"], error.message);
                assert_eq!(
                    document["error"]["position"],
                    serde_json::json!(error.position)
                );
            }
        }
        assert_eq!(
            service.generation(),
            1,
            "failed loads publish no generation"
        );
        std::fs::write(
            service.fixture.path("data.json"),
            case["files"]["data.json"].as_str().unwrap(),
        )
        .unwrap();
        let source = case["files"]["main.ts"]
            .as_str()
            .unwrap()
            .replace(": 44 = 44", ": 45 = 45");
        std::fs::write(service.fixture.path("main.ts"), source).unwrap();
        let repaired = json(service.run(requests[2]));
        assert_eq!(repaired["type"]["summary"], "45");
        assert_eq!(service.generation(), 2);
    }
}

#[cfg(unix)]
#[test]
fn recovered_json_initialization_keeps_service_causes_and_unrelated_queries_independent() {
    use support::service::{Service, json};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-json-initialization");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(root.join("cases.json")).unwrap()).unwrap();
    for profile in ["strict", "loose"] {
        let config = std::fs::read_to_string(root.join(format!("{profile}.json"))).unwrap();
        for name in ["array-missing", "array-wrong-arity"] {
            let case = cases.iter().find(|case| case["name"] == name).unwrap();
            for recovery_first in [false, true] {
                let mut files: Vec<_> = case["files"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(name, text)| (name.as_str(), text.as_str().unwrap()))
                    .collect();
                files.push(("tsconfig.json", &config));
                let service = Service::of(Fixture::new(&files));
                let source = case["files"]["main.ts"].as_str().unwrap();
                let marker = "/*#alias*/";
                let alias = position(source, source.find(marker).unwrap() + marker.len());
                let at = format!("main.ts:{}:{}", alias.line, alias.column);
                let healthy = &["--type", "--name", "healthy", "main.ts", "--json"];
                for recovery in if recovery_first {
                    [true, false, true, false]
                } else {
                    [false, true, false, true]
                } {
                    let label = format!("{profile}/{name}/{recovery_first}/{recovery}");
                    if recovery {
                        let request =
                            &["--type", "--at", &at, "--expand", "--depth", "1", "--json"];
                        let direct = service.run(&[request.to_vec(), vec!["--direct"]].concat());
                        let remote = service.run(request);
                        assert_eq!(remote.status.code(), Some(1), "{label}: {remote:?}");
                        assert_eq!(remote.status.code(), direct.status.code());
                        assert_eq!(remote.stdout, direct.stdout, "{label}");
                        assert_eq!(remote.stderr, direct.stderr, "{label}");
                        assert!(remote.stderr.is_empty());
                        let document: serde_json::Value =
                            serde_json::from_slice(&remote.stdout).unwrap();
                        assert_eq!(document["outcome"], "error");
                        assert_eq!(
                            document["error"]["kind"],
                            serde_json::json!(SemanticErrorKind::InvalidSemantic)
                        );
                        assert_eq!(document["error"]["reason"], "error_type");
                        assert_eq!(
                            document["error"]["path"],
                            serde_json::json!(service.fixture.path("data.json"))
                        );
                        let json_source = case["files"]["data.json"].as_str().unwrap();
                        assert_eq!(
                            document["error"]["position"],
                            serde_json::json!(position(
                                json_source,
                                json_source.find('[').unwrap()
                            ))
                        );
                        let message = if name == "array-missing" {
                            "Cannot find global type 'Array'."
                        } else {
                            "Global type 'Array' must have 1 type parameter(s)."
                        };
                        let cause = document["error"]["causes"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|cause| cause["message"] == message)
                            .unwrap();
                        if name == "array-missing" {
                            assert_eq!(cause["reason"], "unresolved_symbol");
                            assert!(cause["location"].is_null(), "{label}: {cause}");
                        } else {
                            assert_eq!(cause["reason"], "error_type");
                            let globals = case["files"]["globals.d.ts"].as_str().unwrap();
                            let start =
                                globals.find("interface Array").unwrap() + "interface ".len();
                            assert_eq!(
                                cause["location"],
                                serde_json::json!({
                                    "file": "globals.d.ts", "start": position(globals, start),
                                    "end": position(globals, start + "Array".len()),
                                }),
                                "{label}"
                            );
                        }
                        assert!(document["error"]["omittedCauses"].is_null(), "{label}");
                    } else {
                        let direct = service.run(&[healthy.to_vec(), vec!["--direct"]].concat());
                        let remote = service.run(healthy);
                        assert_eq!(remote.stdout, direct.stdout, "{label}");
                        assert_eq!(json(remote)["type"]["summary"], "44", "{label}");
                    }
                    assert_eq!(
                        service.generation(),
                        1,
                        "{label}: recovered initialization publishes one snapshot"
                    );
                }
            }
        }
    }
}

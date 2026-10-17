//! Public UTF-16 text cannot substitute for exact cooked native identity bytes.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{
    AnalysisError, InputErrorKind, SemanticErrorKind, SemanticFailureReason,
};
use typescript_parser::project::Project;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn each_non_scalar_cooked_name_is_rejected_at_its_actual_literal() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/native-string-names");
    for mode in ["strict", "loose"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-{mode}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 27);
        let config = format!(
            r#"{{"compilerOptions":{{"strict":{},"noLib":true}}}}"#,
            mode == "strict"
        );
        for row in rows {
            // Property names retain their existing query-time boundary.
            if row["family"] == "property" {
                continue;
            }
            let name = row["name"].as_str().unwrap();
            let file = row["file"].as_str().unwrap();
            let source = std::fs::read_to_string(root.join(file)).unwrap();
            let ambient_file = format!("ambient-{}.d.ts", row["variant"].as_str().unwrap());
            let ambient = if row["family"] == "module" {
                Some(std::fs::read_to_string(root.join(&ambient_file)).unwrap())
            } else {
                None
            };
            let mut files = vec![("tsconfig.json", config.as_str()), (file, source.as_str())];
            if let Some(ambient) = &ambient {
                files.push((&ambient_file, ambient));
            }
            let fixture = Fixture::new(&files);
            let loaded =
                Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error);
            if row["admitted"] == true {
                assert!(loaded.is_ok(), "{mode}/{name}: {loaded:?}");
            } else {
                let error = loaded.unwrap_err();
                assert_eq!(
                    error.kind,
                    InputErrorKind::Unsupported,
                    "{mode}/{name}: {error}"
                );
                let (error_file, error_source) = match &ambient {
                    Some(ambient) => (ambient_file.as_str(), ambient.as_str()),
                    None => (file, source.as_str()),
                };
                assert_eq!(
                    error.path,
                    Some(fixture.path(error_file)),
                    "{mode}/{name}: {error}"
                );
                assert_eq!(
                    error.position,
                    Some(position(error_source, error_source.find('"').unwrap())),
                    "{mode}/{name}: {error}"
                );
            }
        }
    }
}

#[test]
fn non_scalar_property_names_fail_only_when_the_query_demands_their_identity() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/types/native-string-names");
    for mode in ["strict", "loose"] {
        let rows: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("expected-{mode}.json"))).unwrap(),
        )
        .unwrap();
        let config = format!(
            r#"{{"compilerOptions":{{"strict":{},"target":"ESNext","module":"ESNext","noLib":true}}}}"#,
            mode == "strict"
        );
        for row in rows.into_iter().filter(|row| row["family"] == "property") {
            let file = row["file"].as_str().unwrap();
            let source = std::fs::read_to_string(root.join(file)).unwrap();
            let fixture = Fixture::new(&[
                ("tsconfig.json", &config),
                (file, &source),
                (
                    "globals.d.ts",
                    include_str!("../fixtures/types/native-generated-literal-context/globals.d.ts"),
                ),
            ]);
            let project = fixture.load();
            let marker = "/*#result*/";
            for expand in [false, true, true, false] {
                let answer = project.type_of(TypeRequest {
                    query: TypeQuery::At {
                        file: fixture.path(file),
                        position: position(&source, source.find(marker).unwrap() + marker.len()),
                    },
                    expand,
                    depth: 0,
                });
                if row["admitted"] == true {
                    let Ok(TypeAnswer::Selected(answer)) = answer else {
                        panic!("{mode}/{file}: {answer:?}");
                    };
                    assert_eq!(answer.outcome, TypeOutcome::Found, "{answer:?}");
                    assert_eq!(answer.description.summary, row["summary"].as_str().unwrap());
                    assert!(answer.issues.is_empty());
                } else {
                    let Err(AnalysisError::Semantic(error)) = answer else {
                        panic!("{mode}/{file}: {answer:?}");
                    };
                    assert_eq!(
                        error.kind,
                        SemanticErrorKind::UnsupportedSemantic,
                        "{error:?}"
                    );
                    assert_eq!(
                        error.reason,
                        SemanticFailureReason::UnrepresentableIdentity.into(),
                        "{error:?}"
                    );
                    assert_eq!(error.path, Some(fixture.path(file)));
                    assert_eq!(
                        error.position,
                        Some(position(&source, source.find('"').unwrap())),
                        "{error:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn config_json_and_manifest_names_preserve_their_native_decoder_boundary() {
    let spec: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/types/native-string-json/cases.json"
    ))
    .unwrap();
    let expected: Vec<serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/types/native-string-json/expected-types.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let rows = spec["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 27);
    assert_eq!(expected.len(), 18);
    for row in rows {
        let name = row["name"].as_str().unwrap();
        let (family, variant) = name.split_once('-').unwrap();
        let paired = matches!(variant, "pair" | "scalar");
        let mut config: serde_json::Value =
            serde_json::from_str(spec["config"].as_str().unwrap()).unwrap();
        config["files"]
            .as_array_mut()
            .unwrap()
            .push("globals.d.ts".into());
        let config = config.to_string();
        let mut files = vec![
            ("tsconfig.json", config.as_str()),
            (
                "globals.d.ts",
                include_str!("../fixtures/types/native-generated-literal-context/globals.d.ts"),
            ),
        ];
        files.extend(
            row["files"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(file, text)| (file.as_str(), text.as_str().unwrap())),
        );
        let fixture = Fixture::new(&files);
        let loaded = Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error);
        if family == "config" && !paired {
            let error = loaded.unwrap_err();
            assert_eq!(error.kind, InputErrorKind::Unsupported, "{name}: {error}");
            assert_eq!(
                error.path,
                Some(fixture.path("tsconfig.json")),
                "{name}: {error}"
            );
            let config = row["files"]["tsconfig.json"].as_str().unwrap();
            let start = config.find("\"main.ts\",").unwrap() + "\"main.ts\",".len();
            assert_eq!(
                error.position,
                Some(position(config, start)),
                "{name}: {error}"
            );
            continue;
        }
        let project = loaded.unwrap_or_else(|error| panic!("{name}: {error}"));
        if family == "config" {
            assert!(project.outline(&fixture.path("𐀀.ts")).is_ok(), "{name}");
            assert!(project.outline(&fixture.path("�.ts")).is_err(), "{name}");
            continue;
        }
        let native = expected.iter().find(|item| item["name"] == name).unwrap();
        // Duplicate JSON keys emit grammar diagnostic 1117 but do not consume
        // an errorType or recovered value; the native final type stays usable.
        assert_eq!(
            native["codes"],
            if family == "json" && paired {
                serde_json::json!([1117])
            } else {
                serde_json::json!([])
            },
            "{name}"
        );
        let source = row["files"]["main.ts"].as_str().unwrap();
        for expand in [false, true, true, false] {
            let answer = project.type_of(TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path("main.ts"),
                    position: position(source, source.find("selected").unwrap()),
                },
                expand,
                depth: 2,
            });
            if family == "json" && !paired {
                // Module construction demands every actual key before named
                // selection; exact non-scalar property names remain excluded.
                let Err(AnalysisError::Semantic(error)) = answer else {
                    panic!("{name}: {answer:?}");
                };
                assert_eq!(
                    error.kind,
                    SemanticErrorKind::UnsupportedSemantic,
                    "{name}: {error:?}"
                );
                assert_eq!(
                    error.reason,
                    SemanticFailureReason::UnrepresentableIdentity.into(),
                    "{name}: {error:?}"
                );
                assert_eq!(error.path, Some(fixture.path("data.json")));
                let data = row["files"]["data.json"].as_str().unwrap();
                assert_eq!(
                    error.position,
                    Some(position(data, data.find(',').unwrap() + 1)),
                    "{name}: {error:?}"
                );
                assert!(error.causes.is_empty(), "{name}: {error:?}");
                continue;
            }
            let Ok(TypeAnswer::Selected(answer)) = answer else {
                panic!("{name}: {answer:?}");
            };
            assert_eq!(answer.outcome, TypeOutcome::Found, "{name}: {answer:?}");
            assert_eq!(
                answer.description.summary,
                native["summary"].as_str().unwrap(),
                "{name}"
            );
            assert_eq!(
                serde_json::to_value(&answer.target.location).unwrap(),
                native["location"],
                "{name}"
            );
            assert!(answer.issues.is_empty(), "{name}: {answer:?}");
        }
    }
}

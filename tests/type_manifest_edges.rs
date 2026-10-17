//! Native manifest decoding controls final imported values and the diagnostic
//! acquisition required before a missing alias becomes the native error type.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{
    AnalysisError, InputErrorKind, SemanticErrorKind, SemanticFailureReason,
};
use typescript_parser::project::Project;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn manifest_edges_preserve_native_loaded_values_and_fresh_spelling_acquisition() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-manifest-edges");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(root.join("cases.json")).unwrap()).unwrap();
    let expected: Vec<serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(root.join("expected-workspace.json")).unwrap(),
    )
    .unwrap();
    let template = std::fs::read_to_string(root.join("main.cts")).unwrap();
    let globals = std::fs::read_to_string(root.join("globals.d.ts")).unwrap();
    assert_eq!(cases.len(), expected.len());
    for (case, native) in cases.iter().zip(expected) {
        let name = case["name"].as_str().unwrap();
        assert_eq!(native["name"], name);
        let source = template.replace(
            "from \"pkg\"",
            &format!(
                "from {}",
                serde_json::to_string(case["specifier"].as_str().unwrap_or("pkg")).unwrap()
            ),
        );
        let fixture = Fixture::new(&[
            (
                "tsconfig.json",
                r#"{"compilerOptions":{"strict":true,"noLib":true,"types":[],"module":"NodeNext","moduleResolution":"NodeNext"},"files":["globals.d.ts","main.cts"]}"#,
            ),
            ("package.json", r#"{"type":"commonjs"}"#),
            ("globals.d.ts", &globals),
            ("main.cts", &source),
            (
                "node_modules/pkg/package.json",
                case["manifest"].as_str().unwrap(),
            ),
            (
                "node_modules/pkg/index.d.ts",
                "export declare const present: \"base\";\n",
            ),
            (
                "node_modules/pkg/first.d.ts",
                "export declare const present: \"first\";\n",
            ),
            (
                "node_modules/pkg/second.d.ts",
                "export declare const present: \"second\";\n",
            ),
        ]);
        assert_eq!(
            native["selected"]["errorType"], native["selected"]["unknown"],
            "{name}: native identity and type controls"
        );
        let mode = native["defaultMode"].as_u64().unwrap();
        let first = native["imports"]
            .as_array()
            .unwrap()
            .iter()
            .find(|import| import["resolvedPath"] == native["resolvedPath"])
            .unwrap();
        assert_ne!(
            first["mode"].as_u64().unwrap(),
            mode,
            "{name}: missing alias must demand fresh spelling"
        );
        if native["imports"].as_array().unwrap().last().unwrap()["resolvedPath"].is_null() {
            // Native can answer queries after a missing module diagnostic;
            // this project's required-import contract stops at final loading.
            let loaded =
                Project::load(&fixture.path("tsconfig.json")).map_err(support::input_error);
            let Err(error) = loaded else {
                panic!("{name}: unresolved required module loaded");
            };
            assert_eq!(
                error.kind,
                InputErrorKind::UnresolvedImport,
                "{name}: {error:?}"
            );
            assert_eq!(
                error.path,
                Some(fixture.path("main.cts")),
                "{name}: {error:?}"
            );
            assert_eq!(
                error.position,
                Some(position(&source, source.rfind("from ").unwrap() + 5)),
                "{name}: {error:?}"
            );
            continue;
        }
        for reverse in [false, true] {
            let project = fixture.load();
            let order = if reverse {
                ["missing", "selected", "healthy", "selected"]
            } else {
                ["healthy", "selected", "missing", "selected"]
            };
            for query in order {
                let offset = if query == "healthy" {
                    source.find("healthy =").unwrap()
                } else {
                    let marker = format!("/*#{query}*/");
                    source.find(&marker).unwrap() + marker.len()
                };
                let at = position(&source, offset);
                let error_type = query == "missing"
                    || query == "selected" && native["selected"]["errorType"].as_bool().unwrap();
                for expand in [false, true] {
                    let file = fixture.path("main.cts");
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: file.clone(),
                            position: at,
                        },
                        expand,
                        depth: 0,
                    });
                    if error_type {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{name}/{query}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::InvalidSemantic,
                            "{name}/{query}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::ErrorType.into(),
                            "{name}/{query}: {error:?}"
                        );
                        assert_eq!(error.path, Some(file), "{name}/{query}: {error:?}");
                        assert_eq!(error.position, Some(at), "{name}/{query}: {error:?}");
                        assert!(error.causes.is_empty(), "{name}/{query}: {error:?}");
                    } else {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{name}/{query}: {answer:?}");
                        };
                        assert_eq!(
                            answer.outcome,
                            TypeOutcome::Found,
                            "{name}/{query}: {answer:?}"
                        );
                        let summary = if query == "healthy" {
                            "\"healthy\""
                        } else {
                            native["selected"]["summary"].as_str().unwrap()
                        };
                        assert_eq!(
                            answer.description.summary, summary,
                            "{name}/{query}: {answer:?}"
                        );
                        assert!(answer.issues.is_empty(), "{name}/{query}: {answer:?}");
                    }
                }
            }
        }
    }
}

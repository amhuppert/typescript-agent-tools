//! Native contextual display must complete before alias lookup returns its
//! unknown-symbol result. The native matrix separately records exact spelling;
//! these public queries prove successful acquisition and failure isolation.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn fresh_spelling_finishes_for_native_endings_mappings_and_packages() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/types/native-module-spelling");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(root.join("cases.json")).unwrap()).unwrap();
    let expected: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(root.join("expected.json")).unwrap())
            .unwrap();
    assert_eq!(cases.len(), expected.len());
    let globals = std::fs::read_to_string(root.join("globals.d.ts")).unwrap();
    for (case, native) in cases.iter().zip(expected) {
        let name = case["name"].as_str().unwrap();
        assert_eq!(native["name"], name);
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
            "{name}: must exercise fresh spelling"
        );
        let files = case["files"].as_object().unwrap();
        let mut inputs: Vec<_> = files
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str().unwrap()))
            .collect();
        inputs.push(("globals.d.ts", &globals));
        let fixture = Fixture::new(&inputs);
        let source_name = case["source"].as_str().unwrap();
        let source = files[source_name].as_str().unwrap();
        let marker = "/*#missing*/";
        let missing_at = position(source, source.find(marker).unwrap() + marker.len());
        let (target_name, target_text) = files
            .iter()
            .find(|(_, value)| {
                value
                    .as_str()
                    .unwrap()
                    .starts_with("export declare const present:")
            })
            .unwrap();
        let target_text = target_text.as_str().unwrap();
        let present_at = position(target_text, target_text.find("present").unwrap());
        for reverse in [false, true] {
            let project = fixture.load_config(case["config"].as_str().unwrap());
            for healthy in if reverse {
                [false, true, false, true]
            } else {
                [true, false, true, false]
            } {
                for expand in [false, true, false] {
                    let (file, at) = if healthy {
                        (fixture.path(target_name), present_at)
                    } else {
                        (fixture.path(source_name), missing_at)
                    };
                    let answer = project.type_of(TypeRequest {
                        query: TypeQuery::At {
                            file: file.clone(),
                            position: at,
                        },
                        expand,
                        depth: 0,
                    });
                    if healthy {
                        let Ok(TypeAnswer::Selected(answer)) = answer else {
                            panic!("{name}: {answer:?}");
                        };
                        assert_eq!(answer.outcome, TypeOutcome::Found, "{name}: {answer:?}");
                        assert_eq!(
                            answer.description.summary,
                            native["presentSummary"].as_str().unwrap(),
                            "{name}: {answer:?}"
                        );
                        assert!(answer.issues.is_empty(), "{name}: {answer:?}");
                    } else {
                        let Err(AnalysisError::Semantic(error)) = answer else {
                            panic!("{name}: {answer:?}");
                        };
                        assert_eq!(
                            error.kind,
                            SemanticErrorKind::InvalidSemantic,
                            "{name}: {error:?}"
                        );
                        assert_eq!(
                            error.reason,
                            SemanticFailureReason::ErrorType.into(),
                            "{name}: {error:?}"
                        );
                        assert_eq!(error.path, Some(file), "{name}: {error:?}");
                        assert_eq!(error.position, Some(at), "{name}: {error:?}");
                        assert!(error.causes.is_empty(), "{name}: {error:?}");
                    }
                }
            }
        }
    }
}

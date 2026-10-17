//! Actual raw overload declarations differ from the initialized merge target.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind, SemanticFailureReason};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn initialized_global_this_keeps_raw_declaration_failures_at_the_actual_consumer() {
    let fixture = Fixture::existing(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/types/native-global-this-raw"),
    );
    let native: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-global-this-raw/expected.json"
    ))
    .unwrap();
    assert_eq!(native.len(), 11);
    for row in native {
        let scenario = row["scenario"].as_str().unwrap();
        let name = row["name"].as_str().unwrap();
        let file = if scenario == "overload_reverse" {
            "overload.ts".to_owned()
        } else {
            format!("{scenario}.ts")
        };
        let source = std::fs::read_to_string(fixture.path(&file)).unwrap();
        let project = fixture.load_config(&format!("{scenario}.json"));
        let marker = format!("/*#{name}*/");
        let selected = source.find(&marker).unwrap() + marker.len();
        let raw = row["observations"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(name, _)| name.starts_with("globalThisRawImplementation:"))
            .map(|(_, value)| value);
        assert_eq!(raw.is_some(), name == "badCall");
        if let Some(raw) = raw {
            assert_eq!(raw["rawIsGlobal"], false);
            assert_eq!(raw["rawDeclarations"], 2);
            assert_eq!(raw["mappedToGlobal"], scenario != "local");
            assert_eq!(
                raw["globalDeclarations"],
                if scenario == "local" { 0 } else { 3 }
            );
        }
        for expand in [false, true] {
            let answer = project.type_of(TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path(&file),
                    position: position(&source, selected),
                },
                expand,
                depth: 0,
            });
            if name != "badCall" {
                let Ok(TypeAnswer::Selected(result)) = answer else {
                    panic!("{scenario}/{name}/{expand}: {answer:?}");
                };
                assert_eq!(
                    result.outcome,
                    TypeOutcome::Found,
                    "{scenario}/{name}: {result:?}"
                );
                assert_eq!(result.description.summary, row["summary"].as_str().unwrap());
                assert!(result.issues.is_empty(), "{scenario}/{name}: {result:?}");
                continue;
            }
            // Native constructs the real recovery signature before its raw
            // implementation diagnostic. The retained raw symbol keeps that
            // diagnostic at its actual consumer.
            let Err(AnalysisError::Semantic(error)) = answer else {
                panic!("{scenario}/{name}/{expand}: {answer:?}");
            };
            assert_eq!(
                error.kind,
                SemanticErrorKind::InvalidSemantic,
                "{scenario}: {error:?}"
            );
            assert_eq!(
                error.reason,
                SemanticFailureReason::OverloadRecovery.into(),
                "{scenario}: {error:?}"
            );
            assert_eq!(
                error.position,
                Some(position(&source, source.find("globalThis(123").unwrap()))
            );
            assert_eq!(
                error.path.as_deref(),
                Some(fixture.path(&file).canonicalize().unwrap().as_path())
            );
            // Raw pre-merge metadata is now retained. Native overload recovery
            // remains invalid without the former artificial unsupported cause.
            assert!(error.causes.is_empty(), "{scenario}: {error:?}");
            assert!(error.omitted_causes.is_none());
        }
    }
}
